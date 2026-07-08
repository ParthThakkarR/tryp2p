use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, oneshot, Mutex};
use tracing::{info, warn};

/// A registered peer's pairing channel.
/// The CONNECT handler sends its stream here so the REGISTER handler can bridge both.
struct PeerSlot {
    addr: SocketAddr,
    pair_tx: oneshot::Sender<(TcpStream, SocketAddr)>,
}

type PeerMap = Arc<Mutex<HashMap<String, PeerSlot>>>;

pub struct RelayServer {
    port: u16,
    shutdown: Option<broadcast::Sender<()>>,
}

impl RelayServer {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            shutdown: None,
        }
    }

    pub async fn start(&mut self) -> Result<(), String> {
        let (tx, _) = broadcast::channel(1);
        self.shutdown = Some(tx.clone());

        let peers: PeerMap = Arc::new(Mutex::new(HashMap::new()));
        let listener = TcpListener::bind(format!("0.0.0.0:{}", self.port))
            .await
            .map_err(|e| format!("Bind: {e}"))?;
        info!("Relay server listening on port {}", self.port);

        tokio::spawn(async move {
            loop {
                let mut rx = tx.subscribe();
                let peers = peers.clone();
                tokio::select! {
                    result = listener.accept() => {
                        match result {
                            Ok((stream, addr)) => {
                                tokio::spawn(handle_relay_client(stream, addr, peers));
                            }
                            Err(e) => warn!("Accept error: {e}"),
                        }
                    }
                    _ = rx.recv() => break,
                }
            }
        });

        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
    }
}

async fn handle_relay_client(mut stream: TcpStream, addr: SocketAddr, peers: PeerMap) {
    let mut buf = [0u8; 4096];
    let n = match stream.read(&mut buf).await {
        Ok(n) if n > 0 => n,
        _ => return,
    };
    let msg = String::from_utf8_lossy(&buf[..n]);

    if let Some(peer_id) = msg.strip_prefix("REGISTER:") {
        let peer_id = peer_id.trim().to_string();
        info!("Relay register: {peer_id} from {addr}");
        let _ = stream.write_all(b"RELAY_REGISTERED").await;

        // Create pairing channel — CONNECT handler sends its stream here
        let (pair_tx, mut pair_rx) = oneshot::channel::<(TcpStream, SocketAddr)>();
        {
            let mut map = peers.lock().await;
            map.insert(peer_id.clone(), PeerSlot { addr, pair_tx });
        }

        // Wait for heartbeats, pairing, or timeout (2 missed heartbeats = 60s)
        let mut deadline = tokio::time::Instant::now() + Duration::from_secs(60);
        let mut hb_buf = [0u8; 64];
        loop {
            tokio::select! {
                // Read heartbeats (or detect disconnect)
                result = stream.read(&mut hb_buf) => {
                    match result {
                        Ok(0) | Err(_) => {
                            info!("Relay: peer {peer_id} disconnected");
                            peers.lock().await.remove(&peer_id);
                            break;
                        }
                        Ok(n) => {
                            let msg = String::from_utf8_lossy(&hb_buf[..n]);
                            if msg.contains("HEARTBEAT") {
                                // Reset the 60s deadline on each heartbeat
                                deadline = tokio::time::Instant::now() + Duration::from_secs(60);
                            }
                        }
                    }
                }
                // CONNECT handler paired us — bridge the two streams
                result = &mut pair_rx => {
                    match result {
                        Ok((mut connecting_stream, sender_addr)) => {
                            info!("Relay: pairing {peer_id} with sender from {sender_addr}");
                            
                            // Tell the listener the sender's public IP
                            let msg_to_listener = format!("RELAY_PAIRED:{sender_addr}\n");
                            let _ = stream.write_all(msg_to_listener.as_bytes()).await;
                            
                            // Tell the sender the listener's public IP (stored in our slot)
                            let msg_to_sender = format!("RELAY_PAIRED:{addr}\n");
                            let _ = connecting_stream.write_all(msg_to_sender.as_bytes()).await;
                            
                            // Bridge: copy data bidirectionally
                            let (mut s_read, mut s_write) = stream.split();
                            let (mut c_read, mut c_write) = connecting_stream.split();
                            let c1 = tokio::io::copy(&mut s_read, &mut c_write);
                            let c2 = tokio::io::copy(&mut c_read, &mut s_write);
                            tokio::select! {
                                _ = c1 => {},
                                _ = c2 => {},
                            }
                        }
                        Err(_) => {
                            // pair_tx was dropped (entry removed from map)
                        }
                    }
                    break;
                }
                // No heartbeat for 60s — peer is stale, remove
                _ = tokio::time::sleep_until(deadline) => {
                    warn!("Relay: peer {peer_id} heartbeat timeout (60s) — removing");
                    peers.lock().await.remove(&peer_id);
                    break;
                }
            }
        }
    } else if let Some(target) = msg.strip_prefix("CONNECT:") {
        let target = target.trim().to_string();
        info!("Relay connect request from {addr} for {target}");
        let slot = {
            let mut map = peers.lock().await;
            map.remove(&target)
        };
        match slot {
            Some(slot) => {
                // Send our stream to the waiting REGISTER handler — it handles
                // RELAY_PAIRED + bridging. This task can exit.
                if slot.pair_tx.send((stream, addr)).is_err() {
                    warn!("Relay: REGISTER handler for {target} already exited");
                }
            }
            None => {
                let _ = stream.write_all(b"RELAY_NOT_FOUND").await;
            }
        }
    } else {
        let _ = stream.write_all(b"RELAY_OK").await;
    }
}

pub async fn relay_register(relay_addr: SocketAddr, peer_id: &str) -> Result<TcpStream, String> {
    let socket = crate::network::nat::bind_reusable_socket(0)
        .map_err(|e| format!("Bind reusable socket: {e}"))?;
    let mut stream = socket.connect(relay_addr)
        .await
        .map_err(|e| format!("Relay connect: {e}"))?;
    let request = format!("REGISTER:{peer_id}\n");
    stream
        .write_all(request.as_bytes())
        .await
        .map_err(|e| format!("Relay send: {e}"))?;
    let mut buf = [0u8; 64];
    let n = stream
        .read(&mut buf)
        .await
        .map_err(|e| format!("Relay recv: {e}"))?;
    let response = String::from_utf8_lossy(&buf[..n]);
    if response != "RELAY_REGISTERED" {
        return Err(format!("Relay register rejected: {response}"));
    }
    Ok(stream)
}

/// Wait for the relay server to pair this registered stream with a sender.
/// Sends HEARTBEAT every 30s to keep the relay registration alive.
/// Returns the public SocketAddr of the sender.
pub async fn relay_wait_for_pairing(stream: &mut TcpStream) -> Result<SocketAddr, String> {
    let mut buf = [0u8; 64];
    loop {
        tokio::select! {
            // Wait for server to send RELAY_PAIRED:{addr}
            result = stream.read(&mut buf) => {
                let n = result.map_err(|e| format!("Relay read: {e}"))?;
                if n == 0 {
                    return Err("Relay connection closed while waiting for pairing".into());
                }
                let response = String::from_utf8_lossy(&buf[..n]);
                if let Some(addr_str) = response.strip_prefix("RELAY_PAIRED:") {
                    let addr = addr_str.trim().parse::<SocketAddr>()
                        .map_err(|e| format!("Failed to parse sender addr: {e}"))?;
                    return Ok(addr);
                }
                // Ignore unexpected messages (shouldn't happen, but be robust)
            }
            // Send heartbeat every 30s to keep the NAT mapping and relay registration alive
            _ = tokio::time::sleep(Duration::from_secs(30)) => {
                stream.write_all(b"HEARTBEAT\n").await
                    .map_err(|e| format!("Heartbeat send failed (relay connection lost): {e}"))?;
            }
        }
    }
}

pub async fn relay_connect(relay_addr: SocketAddr, target_id: &str) -> Result<(TcpStream, SocketAddr), String> {
    let socket = crate::network::nat::bind_reusable_socket(0)
        .map_err(|e| format!("Bind reusable socket: {e}"))?;
    let mut stream = socket.connect(relay_addr)
        .await
        .map_err(|e| format!("Relay connect: {e}"))?;
    let request = format!("CONNECT:{target_id}\n");
    stream
        .write_all(request.as_bytes())
        .await
        .map_err(|e| format!("Relay send: {e}"))?;
    let mut buf = [0u8; 64];
    let n = stream
        .read(&mut buf)
        .await
        .map_err(|e| format!("Relay recv: {e}"))?;
    let response = String::from_utf8_lossy(&buf[..n]);
    if let Some(addr_str) = response.strip_prefix("RELAY_PAIRED:") {
        let addr = addr_str.trim().parse::<SocketAddr>()
            .map_err(|e| format!("Failed to parse listener addr: {e}"))?;
        Ok((stream, addr))
    } else {
        Err(format!("Relay connect rejected: {response}"))
    }
}
