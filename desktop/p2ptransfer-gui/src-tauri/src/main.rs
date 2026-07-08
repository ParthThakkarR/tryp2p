#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use p2ptransfer_core::crypto::aead;
use p2ptransfer_core::crypto::ecdh::EcdhKeyExchange;
use p2ptransfer_core::network::{tcp, nat, relay, stun};
use p2ptransfer_core::p2p::discovery::DiscoveryService;
use p2ptransfer_core::transfer::engine::TransferEngine;
use p2ptransfer_core::transfer::resume::TransferResumeManager;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;

async fn setup_stream(stream: &mut tokio::net::TcpStream) {
    let _ = stream.set_nodelay(true);
}

fn log_msg(path: &std::path::Path, msg: &str) {
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut f| {
            use std::io::Write;
            writeln!(f, "{}", msg)
        });
}

struct AppState {
    engine: TransferEngine,
    resume_manager: Arc<TransferResumeManager>,
    discovery: Arc<RwLock<Option<DiscoveryService>>>,
    listener_handle: Arc<RwLock<Option<tauri::async_runtime::JoinHandle<()>>>>,
    shutdown_flag: Arc<AtomicBool>,
}

#[derive(serde::Serialize)]
struct PeerEntry {
    name: String,
    addr: String,
    last_seen: u64,
}

#[derive(serde::Serialize)]
struct TransferEntry {
    id: String,
    file_path: String,
    peer_addr: String,
    file_size: i64,
    bytes_transferred: i64,
    status: String,
}

#[tauri::command]
async fn list_peers(state: tauri::State<'_, AppState>) -> Result<Vec<PeerEntry>, String> {
    let discovery_lock = state.discovery.read().await;
    if let Some(discovery) = discovery_lock.as_ref() {
        let peers = discovery.get_peers().await;
        Ok(peers
            .into_iter()
            .map(|p| PeerEntry {
                name: p.device_name,
                addr: p.socket_addr.to_string(),
                last_seen: p.last_seen_epoch,
            })
            .collect())
    } else {
        Ok(Vec::new())
    }
}

#[tauri::command]
async fn list_transfers(state: tauri::State<'_, AppState>) -> Result<Vec<TransferEntry>, String> {
    let transfers = state
        .resume_manager
        .list_transfers()
        .map_err(|e| e.to_string())?;
    Ok(transfers
        .into_iter()
        .map(|t| TransferEntry {
            id: t.id,
            file_path: t.file_path,
            peer_addr: t.peer_addr,
            file_size: t.file_size,
            bytes_transferred: t.bytes_transferred,
            status: format!("{:?}", t.status),
        })
        .collect())
}

#[tauri::command]
async fn get_config() -> Result<serde_json::Value, String> {
    let config_path = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("p2p")
        .join("config.toml");

    if config_path.exists() {
        let content = std::fs::read_to_string(&config_path).map_err(|e| e.to_string())?;
        toml::from_str(&content).map_err(|e| e.to_string())
    } else {
        Ok(serde_json::json!({
            "tcp_port": 9877,
            "chunk_size": 16777216,
            "compression_level": 10,
            "discovery_port": 9876,
        }))
    }
}

#[tauri::command]
async fn set_config(key: String, value: String) -> Result<(), String> {
    let config_path = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("p2p")
        .join("config.toml");

    let mut config: serde_json::Value = if config_path.exists() {
        let content = std::fs::read_to_string(&config_path).map_err(|e| e.to_string())?;
        toml::from_str(&content).map_err(|e| e.to_string())?
    } else {
        serde_json::json!({})
    };

    if let Some(obj) = config.as_object_mut() {
        obj.insert(
            key,
            serde_json::Value::String(value),
        );
    }

    let toml_str = toml::to_string_pretty(&config).map_err(|e| e.to_string())?;
    std::fs::write(&config_path, toml_str).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn ping() -> Result<String, String> {
    Ok("pong".into())
}

async fn try_connect_fallback(
    peer_addr: SocketAddr,
    relay_addr: Option<&SocketAddr>,
    peer_id_opt: Option<String>,
) -> Result<TcpStream, String> {
    // 1. Try direct connect
    if let Ok(s) = tcp::connect(peer_addr).await {
        return Ok(s);
    }

    // 2. STUN discover our own public address (for diagnostics)
    let our_public = std::thread::spawn(|| stun::discover_address(None))
        .join()
        .ok()
        .and_then(|r| r.ok());
    if let Some(ref info) = our_public {
        eprintln!("  Our public IP: {}", info.public_addr);
    }

    // 3. Try TCP hole punch
    if let Ok(s) = nat::tcp_hole_punch(peer_addr.port(), peer_addr, 5000).await {
        return Ok(s);
    }

    // 4. Try relay if configured
    if let Some(relay) = relay_addr {
        let target_id = peer_id_opt.unwrap_or_else(|| format!("{}:{}", peer_addr.ip(), peer_addr.port()));
        let (relay_stream, peer_public_addr) = relay::relay_connect(*relay, &target_id)
            .await
            .map_err(|e| format!("Relay connection failed: {e}"))?;
            
        let local_port = relay_stream.local_addr().map(|a| a.port()).unwrap_or(0);
        
        if let Ok(direct_stream) = nat::tcp_hole_punch(local_port, peer_public_addr, 5000).await {
            return Ok(direct_stream);
        } else {
            return Ok(relay_stream);
        }
    }

    Err(format!(
        "Cannot reach {peer_addr}.\n\n\
         Options:\n\
         1. Use --relay <host:port> to connect via a relay server\n\
         2. Forward port {} on the receiver's router\n\
         3. Use a VPN like Tailscale/ZeroTier to create a virtual LAN\n\n\
         If the receiver is on the same network, check the IP address.",
        peer_addr.port()
    ))
}


#[tauri::command]
async fn send_file(
    path: String,
    peer: String,
    _compression: i32,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let file_path = std::path::PathBuf::from(&path);
    
    // Resolve peer
    let mut resolved_peer = peer.clone();
    let mut resolved_peer_id = None;
    if let Ok(Some(contact)) = state.resume_manager.get_contact(&peer) {
        resolved_peer = format!("{}:{}", contact.last_known_ip, contact.last_known_port);
        resolved_peer_id = Some(contact.peer_id.clone());
    }
    
    let peer_addr: SocketAddr = resolved_peer.parse().map_err(|e| format!("Invalid peer address: {e}"))?;

    // Hardcode relay config reading for GUI (can be extended to read from config later)
    let config_path = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("p2p")
        .join("config.toml");
    let mut relay_addr = None;
    if config_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            if let Ok(config) = toml::from_str::<serde_json::Value>(&content) {
                if let Some(relay_str) = config.get("relay_server").and_then(|v| v.as_str()) {
                    relay_addr = relay_str.parse::<SocketAddr>().ok();
                }
            }
        }
    }

    let mut stream = try_connect_fallback(peer_addr, relay_addr.as_ref(), resolved_peer_id)
        .await
        .map_err(|e| e.to_string())?;
    
    setup_stream(&mut stream).await;

    let kx = EcdhKeyExchange::new();
    let client_pub = kx.public_key_bytes();
    let mut framed = Vec::with_capacity(1 + client_pub.len());
    framed.push(0x05); // TAG_CLIENT_HELLO
    framed.extend_from_slice(&client_pub);
    tcp::send_message(&mut stream, &framed).await.map_err(|e| e.to_string())?;

    let response = tcp::receive_message(&mut stream).await.map_err(|e| e.to_string())?;
    if response.is_empty() || response[0] != 0x06 {
        return Err("Expected SERVER_HELLO".into());
    }
    let server_pub_bytes: [u8; 32] = response[1..]
        .try_into()
        .map_err(|_| "Invalid server pubkey".to_string())?;
    let shared_secret = kx.derive_shared_secret(&server_pub_bytes).map_err(|e| e.to_string())?;
    let enc_key = aead::derive_encryption_key(&shared_secret, b"P2PTRANSFER_SALT_v1", b"p2ptransfer-v1-encryption")
        .map_err(|e| e.to_string())?;
    let nonce_prefix = aead::generate_nonce_prefix();

    let engine = TransferEngine::new(4);
    let mut metadata = engine
        .create_metadata(&file_path, 16 * 1024 * 1024)
        .await
        .map_err(|e| e.to_string())?;
    metadata.nonce_prefix = nonce_prefix;

    let meta_json = serde_json::to_vec(&metadata).map_err(|e| e.to_string())?;
    let mut meta_frame = Vec::with_capacity(1 + meta_json.len());
    meta_frame.push(0x00); // TAG_METADATA
    meta_frame.extend_from_slice(&meta_json);
    tcp::send_message(&mut stream, &meta_frame)
        .await
        .map_err(|e| e.to_string())?;

    let ack = tcp::receive_message(&mut stream)
        .await
        .map_err(|e| e.to_string())?;
    if ack != b"ACCEPT" {
        return Err("Transfer rejected".into());
    }

    for chunk_index in 0..metadata.total_chunks {
        let chunk_data = engine
            .prepare_chunk(&file_path, &metadata, chunk_index)
            .await
            .map_err(|e| e.to_string())?;
        let nonce = aead::build_nonce(&nonce_prefix, chunk_index);
        let encrypted = aead::encrypt(&enc_key, &nonce, &chunk_data)
            .map_err(|e| e.to_string())?;
        let mut chunk_frame = Vec::with_capacity(5 + encrypted.len());
        chunk_frame.extend_from_slice(&(chunk_index as u32).to_le_bytes());
        chunk_frame.push(0); // not compressed
        chunk_frame.extend_from_slice(&encrypted);
        let mut tagged = Vec::with_capacity(1 + chunk_frame.len());
        tagged.push(0x01); // TAG_CHUNK
        tagged.extend_from_slice(&chunk_frame);
        tcp::send_message(&mut stream, &tagged)
            .await
            .map_err(|e| e.to_string())?;
        let _ack = tcp::receive_message(&mut stream).await.map_err(|e| e.to_string())?;
    }

    let _complete = tcp::receive_message(&mut stream)
        .await
        .map_err(|e| e.to_string())?;

    Ok("Transfer complete".into())
}

#[tauri::command]
async fn start_listening(
    output_dir: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    state.shutdown_flag.store(false, Ordering::SeqCst);

    let listener = TcpListener::bind("0.0.0.0:9877")
        .await
        .map_err(|e| e.to_string())?;
    let shutdown = state.shutdown_flag.clone();
    let log_path = std::path::PathBuf::from(&output_dir).join("p2p_debug.log");
    let log_path2 = log_path.clone();
    let output_dir2 = output_dir.clone();
    let log_path_clone = log_path.clone();
    let handle = tauri::async_runtime::spawn(async move {
        log_msg(&log_path, "Listener started on port 9877");
        loop {
            if shutdown.load(Ordering::SeqCst) {
                log_msg(&log_path, "Shutdown requested");
                break;
            }
            match tokio::time::timeout(
                std::time::Duration::from_secs(1),
                listener.accept(),
            )
            .await
            {
                Ok(Ok((stream, addr))) => {
                    log_msg(&log_path, &format!("Accepted connection from {addr}"));
                    let out = std::path::PathBuf::from(&output_dir2);
                    let log_path3 = log_path2.clone();
                    tauri::async_runtime::spawn(async move {
                        let result = handle_incoming(stream, addr, out).await;
                        match &result {
                            Ok(_) => log_msg(&log_path3, &format!("Transfer from {addr} completed OK")),
                            Err(e) => log_msg(&log_path3, &format!("Transfer from {addr} FAILED: {e}")),
                        }
                    });
                }
                _ => {}
            }
        }
    });

    let config_path = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("p2p")
        .join("config.toml");
    let mut relay_addr = None;
    if config_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            if let Ok(config) = toml::from_str::<serde_json::Value>(&content) {
                if let Some(relay_str) = config.get("relay_server").and_then(|v| v.as_str()) {
                    relay_addr = relay_str.parse::<SocketAddr>().ok();
                }
            }
        }
    }

    if let Some(relay) = relay_addr {
        let hostname = hostname::get()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let out = std::path::PathBuf::from(&output_dir);
        let log_path3 = log_path_clone;
        let shutdown_relay = state.shutdown_flag.clone();
        tauri::async_runtime::spawn(async move {
            log_msg(&log_path3, &format!("Started relay listener connecting to {relay}"));
            loop {
                if shutdown_relay.load(Ordering::SeqCst) {
                    break;
                }
                match relay::relay_register(relay, &hostname).await {
                    Ok(mut stream) => {
                        if let Ok(sender_public_addr) = relay::relay_wait_for_pairing(&mut stream).await {
                            let local_port = stream.local_addr().map(|a| a.port()).unwrap_or(0);
                            let final_stream = match nat::tcp_hole_punch(local_port, sender_public_addr, 5000).await {
                                Ok(direct) => direct,
                                Err(_) => stream,
                            };
                            let o = out.clone();
                            let l3 = log_path3.clone();
                            tauri::async_runtime::spawn(async move {
                                let result = handle_incoming(final_stream, sender_public_addr, o).await;
                                match &result {
                                    Ok(_) => log_msg(&l3, &format!("Relay transfer completed OK")),
                                    Err(e) => log_msg(&l3, &format!("Relay transfer FAILED: {e}")),
                                }
                            });
                        }
                    }
                    Err(e) => {
                        log_msg(&log_path3, &format!("Relay register failed: {e}"));
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    }
                }
            }
        });
    }

    *state.listener_handle.write().await = Some(handle);
    Ok("Listening on port 9877".into())
}

async fn handle_incoming(
    mut stream: tokio::net::TcpStream,
    addr: SocketAddr,
    output_dir: std::path::PathBuf,
) -> Result<(), String> {
    setup_stream(&mut stream).await;
    let send_err = |stream: &mut tokio::net::TcpStream, msg: &str| {
        let _ = tcp::send_message(stream, format!("ERROR:{msg}").as_bytes());
    };

    let data = tcp::receive_message(&mut stream).await.map_err(|e| {
        let msg = format!("Failed to read CLIENT_HELLO from {addr}: {e}");
        eprintln!("{msg}");
        msg
    })?;
    if data.is_empty() || data[0] != 0x05 {
        send_err(&mut stream, "Expected CLIENT_HELLO");
        return Err("Expected CLIENT_HELLO".into());
    }
    let client_pub_bytes: [u8; 32] = data[1..]
        .try_into()
        .map_err(|_| "Invalid client pubkey".to_string())?;
    let kx = EcdhKeyExchange::new();
    let server_pub = kx.public_key_bytes();
    let mut hello_frame = Vec::with_capacity(1 + server_pub.len());
    hello_frame.push(0x06);
    hello_frame.extend_from_slice(&server_pub);
    tcp::send_message(&mut stream, &hello_frame)
        .await
        .map_err(|e| {
            let msg = format!("Failed to send SERVER_HELLO to {addr}: {e}");
            eprintln!("{msg}");
            msg
        })?;
    let shared_secret = kx
        .derive_shared_secret(&client_pub_bytes)
        .map_err(|e| e.to_string())?;
    let enc_key = aead::derive_encryption_key(&shared_secret, b"P2PTRANSFER_SALT_v1", b"p2ptransfer-v1-encryption")
        .map_err(|e| e.to_string())?;

    let meta_data = tcp::receive_message(&mut stream)
        .await
        .map_err(|e| {
            let msg = format!("Failed to read METADATA from {addr}: {e}");
            eprintln!("{msg}");
            msg
        })?;
    if meta_data.is_empty() || meta_data[0] != 0x00 {
        send_err(&mut stream, "Expected METADATA");
        return Err("Expected METADATA".into());
    }
    let metadata: p2ptransfer_core::transfer::engine::TransferMetadata =
        serde_json::from_slice(&meta_data[1..]).map_err(|e| {
            send_err(&mut stream, "Invalid metadata");
            e.to_string()
        })?;
    let nonce_prefix = metadata.nonce_prefix;
    eprintln!("Incoming transfer: {} from {addr}", metadata.file_name);

    tcp::send_message(&mut stream, b"ACCEPT")
        .await
        .map_err(|e| {
            let msg = format!("Failed to send ACCEPT to {addr}: {e}");
            eprintln!("{msg}");
            msg
        })?;

    let output_path = output_dir.join(&metadata.file_name);
    let engine = TransferEngine::new(4);

    for chunk_index in 0..metadata.total_chunks {
        let chunk_data = tcp::receive_message(&mut stream)
            .await
            .map_err(|e| {
                let msg = format!("Failed to read chunk {chunk_index} from {addr}: {e}");
                eprintln!("{msg}");
                msg
            })?;
        if chunk_data.is_empty() || chunk_data[0] != 0x01 {
            send_err(&mut stream, &format!("Expected CHUNK, got tag={}", chunk_data.first().unwrap_or(&0)));
            return Err("Expected CHUNK".into());
        }
        let nonce = aead::build_nonce(&nonce_prefix, chunk_index);
        let decrypted = aead::decrypt(&enc_key, &nonce, &chunk_data[5..])
            .map_err(|e| {
                send_err(&mut stream, "Decryption failed");
                e.to_string()
            })?;
        engine
            .process_received_chunk(&output_path, chunk_index, metadata.chunk_size, &decrypted)
            .await
            .map_err(|e| e.to_string())?;
        let mut ack = Vec::with_capacity(1 + 4);
        ack.push(0x02);
        ack.extend_from_slice(&(chunk_index as u32).to_le_bytes());
        tcp::send_message(&mut stream, &ack)
            .await
            .map_err(|e| {
                let msg = format!("Failed to send ACK to {addr}: {e}");
                eprintln!("{msg}");
                msg
            })?;
    }

    let valid = engine
        .verify_checksum(&output_path, &metadata.checksum)
        .await
        .map_err(|e| e.to_string())?;
    if valid {
        let mut complete = Vec::with_capacity(1 + 32);
        complete.push(0x03);
        complete.extend_from_slice(&metadata.checksum);
        tcp::send_message(&mut stream, &complete)
            .await
            .map_err(|e| {
                let msg = format!("Failed to send COMPLETE to {addr}: {e}");
                eprintln!("{msg}");
                msg
            })?;
        eprintln!("Transfer complete from {addr}: {}", metadata.file_name);
    }

    Ok(())
}

#[tauri::command]
async fn stop_listening(state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.shutdown_flag.store(true, Ordering::SeqCst);
    if let Some(handle) = state.listener_handle.write().await.take() {
        handle.abort();
    }
    Ok(())
}

fn main() {
    tracing_subscriber::fmt().init();

    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("p2p");

    let resume_manager = Arc::new(
        TransferResumeManager::new(data_dir.join("resume"))
            .expect("Failed to init resume manager"),
    );

    tauri::Builder::default()
        .manage(AppState {
            engine: TransferEngine::new(4),
            resume_manager,
            discovery: Arc::new(RwLock::new(None)),
            listener_handle: Arc::new(RwLock::new(None)),
            shutdown_flag: Arc::new(AtomicBool::new(false)),
        })
        .invoke_handler(tauri::generate_handler![
            list_peers,
            list_transfers,
            get_config,
            set_config,
            ping,
            send_file,
            start_listening,
            stop_listening,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
