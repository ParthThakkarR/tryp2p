use anyhow::{Context, Result};
use iroh::{endpoint::presets, protocol::Router, Endpoint};
use flutter_rust_bridge::frb;
use p2ptransfer_core::network::alpn::{
    TransferEvent, TransferProtocol,
    ALPN, PendingRequests
};
use iroh::endpoint::{QuicTransportConfig, VarInt};
use p2ptransfer_core::crypto::identity::derive_secret_key_from_short_id;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use std::path::PathBuf;

lazy_static::lazy_static! {
    static ref IROH_ROUTER: RwLock<Option<Router>> = RwLock::new(None);
    static ref IROH_ENDPOINT: RwLock<Option<Endpoint>> = RwLock::new(None);
    static ref PENDING_REQUESTS: PendingRequests = PendingRequests::new();
    static ref DISCOVERY_SERVICE: RwLock<Option<p2ptransfer_core::p2p::discovery::DiscoveryService>> = RwLock::new(None);
}

pub enum FrbTransferEvent {
    SendStatus { request_id: String, status: String },
    Rejected { request_id: String },
    Error { request_id: String, error: String },
    Incoming { request_id: String, sender_name: String, sender_node_id: String, file_name: String, file_size: u64 },
    Progress { request_id: String, bytes_transferred: u64, total: u64, speed_bytes_per_sec: f64 },
    Complete { request_id: String, file_path: String, blake3_hash: String, elapsed_secs: f64 },
    Cancelled { request_id: String },
}

#[flutter_rust_bridge::frb(sync)]
pub fn generate_random_short_id() -> String {
    p2ptransfer_core::crypto::identity::generate_random_short_id()
}

pub async fn init_backend(
    short_id: String,
    output_dir: String,
    event_sink: crate::frb_generated::StreamSink<FrbTransferEvent>,
) -> Result<()> {
    let secret_key = derive_secret_key_from_short_id(&short_id)?;
    
    let transport = QuicTransportConfig::builder()
        .stream_receive_window(VarInt::from_u32(256 * 1024 * 1024))
        .receive_window(VarInt::from_u32(1024 * 1024 * 1024))
        .send_window(1024 * 1024 * 1024)
        .initial_mtu(1400)
        .build();

    let endpoint = Endpoint::builder(presets::N0)
        .secret_key(secret_key)
        .alpns(vec![ALPN.to_vec()])
        .transport_config(transport)
        .bind()
        .await
        .context("Failed to bind iroh endpoint")?;
        
    let (tx, mut rx) = mpsc::channel(100);
    
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            let mapped = match event {
                TransferEvent::SendStatus(e) => FrbTransferEvent::SendStatus { request_id: e.request_id, status: e.status },
                TransferEvent::Rejected(e) => FrbTransferEvent::Rejected { request_id: e.request_id },
                TransferEvent::Error(e) => FrbTransferEvent::Error { request_id: e.request_id, error: e.error },
                TransferEvent::Incoming(e) => FrbTransferEvent::Incoming { request_id: e.request_id, sender_name: e.sender_name, sender_node_id: e.sender_node_id, file_name: e.file_name, file_size: e.file_size },
                TransferEvent::Progress(e) => FrbTransferEvent::Progress { request_id: e.request_id, bytes_transferred: e.bytes_transferred, total: e.total, speed_bytes_per_sec: e.speed_bytes_per_sec },
                TransferEvent::Complete(e) => FrbTransferEvent::Complete { request_id: e.request_id, file_path: e.file_path, blake3_hash: e.blake3_hash, elapsed_secs: e.elapsed_secs },
                TransferEvent::Cancelled { request_id } => FrbTransferEvent::Cancelled { request_id },
            };
            let _ = event_sink.add(mapped);
        }
    });
    
    let tx_clone = tx.clone();
    let output_dir_tcp = output_dir.clone();
    let pending_tcp = PENDING_REQUESTS.clone();
    tokio::spawn(async move {
        if let Ok(listener) = tokio::net::TcpListener::bind("0.0.0.0:9877").await {
            while let Ok((stream, addr)) = listener.accept().await {
                let out = std::path::PathBuf::from(&output_dir_tcp);
                let tx_c = tx_clone.clone();
                let pending_c = pending_tcp.clone();
                tokio::spawn(async move {
                    let _ = p2ptransfer_core::network::tcp_compat::handle_incoming(stream, addr, out, &tx_c, pending_c).await;
                });
            }
        }
    });

    if let Ok(mut discovery) = p2ptransfer_core::p2p::discovery::DiscoveryService::new_with_node_id("Mobile".to_string(), 9877, 9876, Some(short_id.clone())).await {
        if discovery.start().await.is_ok() {
            *DISCOVERY_SERVICE.write().await = Some(discovery);
        }
    }

    let protocol = TransferProtocol {
        pending: PENDING_REQUESTS.clone(),
        event_tx: tx,
        output_dir: Arc::new(RwLock::new(output_dir)),
    };
    
    let router = Router::builder(endpoint.clone())
        .accept(ALPN, protocol)
        .spawn();
        
    *IROH_ENDPOINT.write().await = Some(endpoint);
    *IROH_ROUTER.write().await = Some(router);
    Ok(())
}

#[flutter_rust_bridge::frb(sync)]
pub fn set_output_dir(_dir: String) {
    if let Some(_router) = IROH_ROUTER.try_read().unwrap().as_ref() {
    }
}

pub async fn respond_to_transfer(request_id: String, accepted: bool) -> Result<()> {
    PENDING_REQUESTS.respond(&request_id, accepted).await;
    Ok(())
}

/// Pause an active transfer (both TCP and QUIC paths register their flags).
pub async fn pause_transfer(request_id: String) -> Result<()> {
    if let Some(flag) = p2ptransfer_core::network::alpn::TRANSFER_PAUSE_FLAGS.get(&request_id) {
        flag.pause();
    }
    Ok(())
}

/// Resume a paused transfer.
pub async fn resume_transfer(request_id: String) -> Result<()> {
    if let Some(flag) = p2ptransfer_core::network::alpn::TRANSFER_PAUSE_FLAGS.get(&request_id) {
        flag.resume();
    }
    Ok(())
}

/// Cancel an active transfer.
pub async fn cancel_transfer(request_id: String) -> Result<()> {
    if let Some(flag) = p2ptransfer_core::network::alpn::TRANSFER_PAUSE_FLAGS.get(&request_id) {
        flag.cancel();
    }
    Ok(())
}

pub async fn check_peer_online(short_id: String) -> bool {
    let norm_id = short_id.replace("-", "").trim().to_uppercase();
    // ── Path 1: LAN TCP ping (fast) ──────────────────────────────────────
    if let Some(discovery) = DISCOVERY_SERVICE.read().await.as_ref() {
        let peers = discovery.get_peers().await;
        if let Some(peer) = peers.into_iter().find(|p| {
            p.node_id.as_ref().map_or(false, |id| id.replace("-", "").trim().to_uppercase() == norm_id) ||
            p.device_name.eq_ignore_ascii_case(&short_id)
        }) {
            let addr = peer.socket_addr;
            let reachable = tokio::time::timeout(
                std::time::Duration::from_secs(3),
                tokio::net::TcpStream::connect(addr),
            ).await.map(|r| r.is_ok()).unwrap_or(false);
            if reachable {
                return true;
            }
        }
    }

    // ── Path 2: WAN iroh ALPN ping ────────────────────────────────────────
    if let Some(endpoint) = IROH_ENDPOINT.read().await.clone() {
        if let Ok(target_secret_key) = p2ptransfer_core::crypto::identity::derive_secret_key_from_short_id(&short_id) {
            let target_node_id = target_secret_key.public();
            let target_addr = iroh::EndpointAddr::from(target_node_id);
            let result = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                endpoint.connect(target_addr, p2ptransfer_core::network::alpn::ALPN),
            ).await;
            return result.map(|r| r.is_ok()).unwrap_or(false);
        }
    }

    false
}

pub async fn send_file(
    request_id: String,
    target_short_id: String,
    file_path: String,
    sender_name: String,
    event_sink: crate::frb_generated::StreamSink<FrbTransferEvent>,
) -> Result<String> {
    let endpoint = IROH_ENDPOINT.read().await.clone().context("Backend not initialized")?;
    
    let target_secret_key = derive_secret_key_from_short_id(&target_short_id)?;
    let target_node_id = target_secret_key.public();
    let mut target_addr = iroh::EndpointAddr::from(target_node_id);
    
    let (tx, mut rx) = mpsc::channel(100);
    let tx_clone = tx.clone();
    
    let handle = tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            let mapped = match event {
                TransferEvent::SendStatus(e) => FrbTransferEvent::SendStatus { request_id: e.request_id, status: e.status },
                TransferEvent::Rejected(e) => FrbTransferEvent::Rejected { request_id: e.request_id },
                TransferEvent::Error(e) => FrbTransferEvent::Error { request_id: e.request_id, error: e.error },
                TransferEvent::Incoming(e) => FrbTransferEvent::Incoming { request_id: e.request_id, sender_name: e.sender_name, sender_node_id: e.sender_node_id, file_name: e.file_name, file_size: e.file_size },
                TransferEvent::Progress(e) => FrbTransferEvent::Progress { request_id: e.request_id, bytes_transferred: e.bytes_transferred, total: e.total, speed_bytes_per_sec: e.speed_bytes_per_sec },
                TransferEvent::Complete(e) => FrbTransferEvent::Complete { request_id: e.request_id, file_path: e.file_path, blake3_hash: e.blake3_hash, elapsed_secs: e.elapsed_secs },
                TransferEvent::Cancelled { request_id } => FrbTransferEvent::Cancelled { request_id },
            };
            let _ = event_sink.add(mapped);
        }
    });
    
    let mut tcp_success = false;
    let mut final_hash = String::new();
    let norm_target_id = target_short_id.replace("-", "").trim().to_uppercase();
    
    if let Some(discovery) = DISCOVERY_SERVICE.read().await.as_ref() {
        let peers = discovery.get_peers().await;
        if let Some(peer) = peers.into_iter().find(|p| {
            p.node_id.as_ref().map_or(false, |id| id.replace("-", "").trim().to_uppercase() == norm_target_id) ||
            p.device_name.eq_ignore_ascii_case(&target_short_id)
        }) {
            let target_ip = peer.socket_addr.ip().to_string();
            let target_port = peer.socket_addr.port();

            if let Ok(sa) = format!("{}:{}", target_ip, target_port).parse::<std::net::SocketAddr>() {
                target_addr = target_addr.with_ip_addr(sa);
            }
            
            match p2ptransfer_core::network::tcp_compat::send_file_direct_tcp(
                request_id.clone(),
                file_path.clone(),
                &target_ip,
                target_port,
                &tx_clone,
                false,
            ).await {
                Ok(hash) => {
                    tcp_success = true;
                    final_hash = hash;
                }
                Err(e) if e.contains("rejected") || e == "REJECTED" || e.contains("cancelled") || e.contains("Cancelled") => {
                    return Err(anyhow::anyhow!(e));
                }
                Err(e) => {
                    eprintln!("Direct TCP attempt failed ({e}), attempting QUIC direct/relay");
                }
            }
        }
    }
    
    if tcp_success {
        let _ = tx_clone.send(TransferEvent::Complete(p2ptransfer_core::network::alpn::TransferCompleteEvent {
            request_id: request_id.clone(),
            file_path: file_path.clone(),
            blake3_hash: final_hash.clone(),
            elapsed_secs: 0.0,
        })).await;
        drop(tx_clone);
        let _ = handle.await;
        return Ok(final_hash);
    }
    
    match p2ptransfer_core::network::alpn::send_file_to_peer(
        request_id.clone(),
        &endpoint,
        target_addr,
        &PathBuf::from(&file_path),
        &sender_name,
        &tx_clone,
        false
    ).await {
        Ok(hash) => {
            let _ = tx_clone.send(TransferEvent::Complete(p2ptransfer_core::network::alpn::TransferCompleteEvent {
                request_id: request_id.clone(),
                file_path: file_path.clone(),
                blake3_hash: hash.clone(),
                elapsed_secs: 0.0,
            })).await;
            drop(tx_clone);
            let _ = handle.await;
            Ok(hash)
        }
        Err(e) => Err(e),
    }
}
