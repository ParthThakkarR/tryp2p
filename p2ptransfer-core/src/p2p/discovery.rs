use anyhow::{Context, Result};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::peer::PeerInfo;
use super::protocol::{
    BEACON_INTERVAL_SECS, Beacon, DEFAULT_DISCOVERY_PORT, MULTICAST_GROUP, PEER_TIMEOUT_SECS,
};

pub struct DiscoveryService {
    device_name: String,
    tcp_port: u16,
    socket: Arc<UdpSocket>,
    peers: Arc<RwLock<HashMap<SocketAddr, PeerInfo>>>,
    running: Arc<std::sync::atomic::AtomicBool>,
    handles: Vec<tokio::task::JoinHandle<()>>,
}

impl DiscoveryService {
    pub async fn new(device_name: String, tcp_port: u16, discovery_port: u16) -> Result<Self> {
        let bind_addr: SocketAddr = format!("0.0.0.0:{discovery_port}")
            .parse()
            .context("Invalid bind address")?;
        let socket = UdpSocket::bind(bind_addr)
            .await
            .context("Failed to bind UDP socket on discovery port")?;

        let multicast_ip: std::net::Ipv4Addr = MULTICAST_GROUP
            .parse()
            .context("Invalid multicast group address")?;
        socket
            .join_multicast_v4(multicast_ip, std::net::Ipv4Addr::UNSPECIFIED)
            .context("Failed to join multicast group")?;

        info!(
            "Discovery service bound to 0.0.0.0:{DEFAULT_DISCOVERY_PORT}, joined {MULTICAST_GROUP}"
        );

        Ok(Self {
            device_name,
            tcp_port,
            socket: Arc::new(socket),
            peers: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            handles: Vec::new(),
        })
    }

    pub async fn start(&mut self) -> Result<()> {
        self.running
            .store(true, std::sync::atomic::Ordering::SeqCst);
        info!("Discovery service started");

        let peers = self.peers.clone();
        let socket = self.socket.clone();
        let running = self.running.clone();
        let beacon = Beacon {
            device_name: self.device_name.clone(),
            p2ptransfer_version: env!("CARGO_PKG_VERSION").to_string(),
            tcp_port: self.tcp_port,
        };

        // Beacon broadcaster
        let handle = {
            let socket = socket.clone();
            let running = running.clone();
            let beacon_bytes =
                serde_json::to_vec(&beacon).expect("Beacon serialization should never fail");
            let multicast_addr: SocketAddr = format!("{MULTICAST_GROUP}:{DEFAULT_DISCOVERY_PORT}")
                .parse()
                .expect("Hardcoded multicast address should be valid");

            tokio::spawn(async move {
                while running.load(std::sync::atomic::Ordering::SeqCst) {
                    if let Err(e) = socket.send_to(&beacon_bytes, multicast_addr).await {
                        error!("Failed to send beacon: {e}");
                    } else {
                        debug!("Beacon sent to {multicast_addr}");
                    }
                    tokio::time::sleep(Duration::from_secs(BEACON_INTERVAL_SECS)).await;
                }
            })
        };
        self.handles.push(handle);

        // Listener
        let handle = {
            let socket = socket.clone();
            let peers = peers.clone();
            let running = running.clone();

            tokio::spawn(async move {
                let mut buf = [0u8; 2048];
                while running.load(std::sync::atomic::Ordering::SeqCst) {
                    match socket.recv_from(&mut buf).await {
                        Ok((len, src)) => {
                            let data = &buf[..len];
                            match serde_json::from_slice::<Beacon>(data) {
                                Ok(beacon) => {
                                    debug!("Received beacon from {} ({})", beacon.device_name, src);
                                    let mut peers = peers.write().await;
                                    let peer_addr = SocketAddr::new(src.ip(), beacon.tcp_port);
                                    match peers.get_mut(&peer_addr) {
                                        Some(peer) => peer.touch(),
                                        None => {
                                            let peer = PeerInfo::new(
                                                beacon.device_name.clone(),
                                                beacon.tcp_port,
                                                peer_addr,
                                            );
                                            info!(
                                                "Discovered new peer: {} at {}",
                                                peer.device_name, peer_addr
                                            );
                                            peers.insert(peer_addr, peer);
                                        }
                                    }
                                }
                                Err(e) => debug!("Invalid beacon from {src}: {e}"),
                            }
                        }
                        Err(e) => {
                            warn!("Discovery recv error: {e}");
                        }
                    }
                }
            })
        };
        self.handles.push(handle);

        // Peer cleanup
        let handle = {
            let peers = peers.clone();
            let running = running.clone();
            tokio::spawn(async move {
                while running.load(std::sync::atomic::Ordering::SeqCst) {
                    tokio::time::sleep(Duration::from_secs(10)).await;
                    let mut peers = peers.write().await;
                    let before = peers.len();
                    peers.retain(|_, peer| !peer.is_stale(Duration::from_secs(PEER_TIMEOUT_SECS)));
                    let removed = before - peers.len();
                    if removed > 0 {
                        debug!("Cleaned up {removed} stale peer(s)");
                    }
                }
            })
        };
        self.handles.push(handle);

        Ok(())
    }

    pub async fn stop(&mut self) {
        self.running
            .store(false, std::sync::atomic::Ordering::SeqCst);
        for handle in self.handles.drain(..) {
            let _ = handle.await;
        }
        info!("Discovery service stopped");
    }

    pub async fn get_peers(&self) -> Vec<PeerInfo> {
        self.peers.read().await.values().cloned().collect()
    }

    pub async fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl Drop for DiscoveryService {
    fn drop(&mut self) {
        self.running
            .store(false, std::sync::atomic::Ordering::SeqCst);
        for handle in self.handles.drain(..) {
            handle.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_peer_list_insert_and_query() {
        let peers: Arc<RwLock<HashMap<SocketAddr, PeerInfo>>> =
            Arc::new(RwLock::new(HashMap::new()));

        let p1 = PeerInfo::new(
            "DeviceAlpha".into(),
            9877,
            "192.168.1.10:9877".parse().unwrap(),
        );
        let p2 = PeerInfo::new(
            "DeviceBeta".into(),
            9878,
            "192.168.1.20:9878".parse().unwrap(),
        );

        peers.write().await.insert(p1.socket_addr, p1.clone());
        peers.write().await.insert(p2.socket_addr, p2.clone());

        let all = peers.read().await;
        assert_eq!(all.len(), 2);
        assert!(all.contains_key(&"192.168.1.10:9877".parse().unwrap()));
        assert!(all.contains_key(&"192.168.1.20:9878".parse().unwrap()));
    }

    #[tokio::test]
    async fn test_peer_touch_updates_last_seen() {
        let mut peer = PeerInfo::new("Test".into(), 9877, "127.0.0.1:9877".parse().unwrap());
        peer.last_seen_epoch = 0;
        peer.touch();
        assert!(
            peer.last_seen_epoch > 0,
            "touch() should set last_seen_epoch to a non-zero value"
        );
    }

    #[tokio::test]
    async fn test_peer_staleness() {
        let mut peer = PeerInfo::new("StaleTest".into(), 9877, "127.0.0.1:9877".parse().unwrap());
        peer.last_seen_epoch = 0;
        assert!(peer.is_stale(Duration::from_secs(10)));
        peer.touch();
        assert!(!peer.is_stale(Duration::from_secs(3600)));
    }

    #[tokio::test]
    async fn test_cleanup_removes_stale_peers() {
        let peers: Arc<RwLock<HashMap<SocketAddr, PeerInfo>>> =
            Arc::new(RwLock::new(HashMap::new()));

        let mut fresh = PeerInfo::new("Fresh".into(), 9877, "10.0.0.1:9877".parse().unwrap());
        fresh.touch();

        let mut stale = PeerInfo::new("Stale".into(), 9877, "10.0.0.2:9877".parse().unwrap());
        stale.last_seen_epoch = 0;

        peers.write().await.insert(fresh.socket_addr, fresh);
        peers.write().await.insert(stale.socket_addr, stale);

        {
            let mut guard = peers.write().await;
            guard.retain(|_, p| !p.is_stale(Duration::from_secs(30)));
        }

        assert_eq!(peers.read().await.len(), 1);
        assert!(
            peers
                .read()
                .await
                .contains_key(&"10.0.0.1:9877".parse().unwrap())
        );
    }
}
