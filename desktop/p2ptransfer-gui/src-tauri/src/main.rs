#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod identity;
mod protocol;

use p2ptransfer_core::crypto::aead;
use p2ptransfer_core::crypto::ecdh::EcdhKeyExchange;
use p2ptransfer_core::network::tcp;
use p2ptransfer_core::p2p::discovery::DiscoveryService;
use p2ptransfer_core::transfer::engine::TransferEngine;
use p2ptransfer_core::transfer::resume::TransferResumeManager;
use p2ptransfer_core::network::wan::runner::WanTunnel;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::net::{TcpListener, TcpStream};
use tauri::Manager;
use tokio::sync::RwLock;

// Iroh identity-based transport
use iroh::{Endpoint, endpoint::presets, protocol::Router};
use iroh::endpoint::{QuicTransportConfig, VarInt};

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
    data_dir: std::path::PathBuf,
    // Identity-based iroh transport
    iroh_endpoint: Arc<Endpoint>,
    iroh_router: Arc<RwLock<Option<Router>>>,
    device_id: String,
    device_name: String,
    pending_requests: protocol::PendingRequests,
    output_dir: Arc<RwLock<String>>,
    wan_tunnel: Arc<tokio::sync::Mutex<Option<WanTunnel>>>,
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

#[derive(serde::Serialize)]
struct ContactEntry {
    name: String,
    node_id: String,
}

/* ── Existing LAN commands (preserved) ─────────────────────── */

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
fn pause_transfer(request_id: String) {
    let flag = protocol::TRANSFER_PAUSE_FLAGS
        .entry(request_id)
        .or_insert_with(protocol::PauseFlag::new)
        .clone();
    flag.pause();
}

#[tauri::command]
fn resume_transfer(request_id: String) {
    if let Some(flag) = protocol::TRANSFER_PAUSE_FLAGS.get(&request_id) {
        flag.resume();
    }
}

/// Returns all request IDs currently awaiting user acceptance.
/// Used by the frontend on reconnect to re-display any pending popups.
#[tauri::command]
async fn get_pending_transfers(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, String> {
    Ok(state.pending_requests.pending_ids().await)
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
        obj.insert(key, serde_json::Value::String(value));
    }

    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
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
    _relay_addr: Option<&SocketAddr>,
    _peer_id_opt: Option<String>,
) -> Result<TcpStream, String> {
    let direct_result = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        tcp::connect(peer_addr),
    ).await;
    if let Ok(Ok(s)) = direct_result {
        return Ok(s);
    }

    Err(format!(
        "Cannot reach {peer_addr}.\n\n\
         Use the WAN mode (Send → pick contact) for cross-network transfers.\n\
         For LAN, check that the receiver is listening and the IP is correct."
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

    let mut resolved_peer = peer.clone();
    let mut resolved_peer_id = None;
    if let Ok(Some(contact)) = state.resume_manager.get_contact(&peer) {
        resolved_peer = format!("{}:{}", contact.last_known_ip, contact.last_known_port);
        resolved_peer_id = Some(contact.peer_id.clone());
    }

    let peer_addr: SocketAddr = resolved_peer.parse()
        .map_err(|_| format!("Cannot resolve '{}'", peer))?;

    let mut stream = try_connect_fallback(peer_addr, None, resolved_peer_id)
        .await
        .map_err(|e| e.to_string())?;

    setup_stream(&mut stream).await;

    let kx = EcdhKeyExchange::new();
    let client_pub = kx.public_key_bytes();
    let mut framed = Vec::with_capacity(1 + client_pub.len());
    framed.push(0x05);
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
    meta_frame.push(0x00);
    meta_frame.extend_from_slice(&meta_json);
    tcp::send_message(&mut stream, &meta_frame).await.map_err(|e| e.to_string())?;

    let ack = tcp::receive_message(&mut stream).await.map_err(|e| e.to_string())?;
    if ack != b"ACCEPT" {
        return Err("Transfer rejected".into());
    }

    for chunk_index in 0..metadata.total_chunks {
        let chunk_data = engine
            .prepare_chunk(&file_path, &metadata, chunk_index)
            .await
            .map_err(|e| e.to_string())?;
        let nonce = aead::build_nonce(&nonce_prefix, chunk_index);
        let encrypted = aead::encrypt(&enc_key, &nonce, &chunk_data).map_err(|e| e.to_string())?;
        let mut chunk_frame = Vec::with_capacity(5 + encrypted.len());
        chunk_frame.extend_from_slice(&(chunk_index as u32).to_le_bytes());
        chunk_frame.push(0);
        chunk_frame.extend_from_slice(&encrypted);
        let mut tagged = Vec::with_capacity(1 + chunk_frame.len());
        tagged.push(0x01);
        tagged.extend_from_slice(&chunk_frame);
        tcp::send_message(&mut stream, &tagged).await.map_err(|e| e.to_string())?;
        let _ack = tcp::receive_message(&mut stream).await.map_err(|e| e.to_string())?;
    }

    let _complete = tcp::receive_message(&mut stream).await.map_err(|e| e.to_string())?;
    Ok("Transfer complete".into())
}

#[tauri::command]
async fn start_listening(
    output_dir: String,
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
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
    let app_handle_1 = app_handle.clone();
    let handle = tauri::async_runtime::spawn(async move {
        log_msg(&log_path_clone, "Listener started on port 9877");
        loop {
            if shutdown.load(Ordering::SeqCst) {
                log_msg(&log_path_clone, "Shutdown requested");
                break;
            }
            match tokio::time::timeout(
                std::time::Duration::from_secs(1),
                listener.accept(),
            )
            .await
            {
                Ok(Ok((stream, addr))) => {
                    log_msg(&log_path_clone, &format!("Accepted connection from {addr}"));
                    let out = std::path::PathBuf::from(&output_dir2);
                    let log_path3 = log_path2.clone();
                    let app_h = app_handle_1.clone();
                    tauri::async_runtime::spawn(async move {
                        let result = handle_incoming(stream, addr, out, app_h).await;
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

    *state.listener_handle.write().await = Some(handle);
    Ok("Listening on port 9877".into())
}

async fn handle_incoming(
    mut stream: tokio::net::TcpStream,
    addr: SocketAddr,
    output_dir: std::path::PathBuf,
    app_handle: tauri::AppHandle,
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
    tcp::send_message(&mut stream, &hello_frame).await.map_err(|e| {
        let msg = format!("Failed to send SERVER_HELLO to {addr}: {e}");
        eprintln!("{msg}");
        msg
    })?;
    let shared_secret = kx.derive_shared_secret(&client_pub_bytes).map_err(|e| e.to_string())?;
    let enc_key = aead::derive_encryption_key(&shared_secret, b"P2PTRANSFER_SALT_v1", b"p2ptransfer-v1-encryption")
        .map_err(|e| e.to_string())?;

    let meta_data = tcp::receive_message(&mut stream).await.map_err(|e| {
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

    let (tx, rx) = tokio::sync::oneshot::channel();
    let msg = format!("Incoming transfer:\nFile: {}\nSize: {} bytes\nFrom: {}\n\nAccept?", metadata.file_name, metadata.file_size, addr);
    tauri::api::dialog::ask(
        app_handle.get_window("main").as_ref(),
        "Transfer Request",
        msg,
        move |answer| {
            let _ = tx.send(answer);
        }
    );

    let answer = match tokio::time::timeout(std::time::Duration::from_secs(30), rx).await {
        Ok(Ok(true)) => true,
        _ => false,
    };

    if !answer {
        send_err(&mut stream, "timeout");
        return Err("Transfer rejected".into());
    }

    tcp::send_message(&mut stream, b"ACCEPT").await.map_err(|e| {
        let msg = format!("Failed to send ACCEPT to {addr}: {e}");
        eprintln!("{msg}");
        msg
    })?;

    let output_path = output_dir.join(&metadata.file_name);
    let engine = TransferEngine::new(4);

    for chunk_index in 0..metadata.total_chunks {
        let chunk_data = tcp::receive_message(&mut stream).await.map_err(|e| {
            let msg = format!("Failed to read chunk {chunk_index} from {addr}: {e}");
            eprintln!("{msg}");
            msg
        })?;
        if chunk_data.is_empty() || chunk_data[0] != 0x01 {
            send_err(&mut stream, &format!("Expected CHUNK, got tag={}", chunk_data.first().unwrap_or(&0)));
            return Err("Expected CHUNK".into());
        }
        let nonce = aead::build_nonce(&nonce_prefix, chunk_index);
        let decrypted = aead::decrypt(&enc_key, &nonce, &chunk_data[5..]).map_err(|e| {
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
        tcp::send_message(&mut stream, &ack).await.map_err(|e| {
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
        tcp::send_message(&mut stream, &complete).await.map_err(|e| {
            let msg = format!("Failed to send COMPLETE to {addr}: {e}");
            eprintln!("{msg}");
            msg
        })?;
        eprintln!("Transfer complete from {addr}: {}", metadata.file_name);
    }

    Ok(())
}

#[tauri::command]
async fn get_default_download_dir() -> Result<String, String> {
    let dir = dirs::download_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .to_string_lossy()
        .to_string();
    Ok(dir)
}

#[tauri::command]
async fn stop_listening(state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.shutdown_flag.store(true, Ordering::SeqCst);
    if let Some(handle) = state.listener_handle.write().await.take() {
        handle.abort();
    }
    Ok(())
}

/* ── Identity-based commands ───────────────────────────────── */

/// Returns this device's permanent ID (iroh NodeId).
#[tauri::command]
async fn get_device_id(state: tauri::State<'_, AppState>) -> Result<String, String> {
    Ok(state.device_id.clone())
}

/// Returns the device display name.
#[tauri::command]
async fn get_device_name(state: tauri::State<'_, AppState>) -> Result<String, String> {
    Ok(state.device_name.clone())
}

/* ── Contact management commands ───────────────────────────── */

#[tauri::command]
async fn add_contact(
    name: String,
    node_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state
        .resume_manager
        .upsert_contact(&name, &node_id, "", 0)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_contacts(state: tauri::State<'_, AppState>) -> Result<Vec<ContactEntry>, String> {
    let contacts = state
        .resume_manager
        .list_contacts()
        .map_err(|e| e.to_string())?;
    Ok(contacts
        .into_iter()
        .map(|c| ContactEntry {
            name: c.name,
            node_id: c.peer_id,
        })
        .collect())
}

#[tauri::command]
async fn remove_contact(
    name: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state
        .resume_manager
        .delete_contact(&name)
        .map_err(|e| e.to_string())
}

/* ── Identity-based transfer commands ─────────────────────── */

/// Send a file to a contact by their name (looks up NodeId from contacts DB).
#[tauri::command]
async fn send_to_contact(
    request_id: String,
    path: String,
    contact_name: String,
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    // Look up contact
    let contact = state
        .resume_manager
        .get_contact(&contact_name)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Contact '{}' not found", contact_name))?;

    // Parse NodeId
    let node_id: iroh::PublicKey = contact
        .peer_id
        .parse()
        .map_err(|e| format!("Invalid NodeId for contact '{}': {}", contact_name, e))?;

    let file_path = std::path::PathBuf::from(&path);
    if !file_path.exists() {
        return Err(format!("File not found: {}", path));
    }

    // Send via iroh QUIC
    let hash = protocol::send_file_to_peer(
        request_id,
        &state.iroh_endpoint,
        node_id,
        &file_path,
        &state.device_name,
        &app_handle,
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(hash)
}

/// Respond to an incoming transfer request (accept or reject).
#[tauri::command]
async fn respond_to_transfer(
    request_id: String,
    accept: bool,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let found = state.pending_requests.respond(&request_id, accept).await;
    if !found {
        return Err(format!("No pending request with id '{}'", request_id));
    }
    Ok(())
}

/// Update the output directory for incoming transfers.
#[tauri::command]
async fn set_output_dir(
    dir: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    *state.output_dir.write().await = dir;
    Ok(())
}

#[tauri::command]
async fn start_wan_tunnel(path: String, state: tauri::State<'_, AppState>) -> Result<String, String> {
    use p2ptransfer_core::network::wan::downloader::ensure_binary;
    use p2ptransfer_core::network::wan_constants::{get_cloudflared_pin, get_copyparty_pin};
    
    let path_buf = std::path::PathBuf::from(&path);
    if !path_buf.exists() {
        return Err("File does not exist".to_string());
    }

    let bin_dir = state.data_dir.join("bin");
    let share_dir = state.data_dir.join("wan_share");
    let _ = std::fs::remove_dir_all(&share_dir);
    std::fs::create_dir_all(&share_dir).map_err(|e| e.to_string())?;
    
    let file_name = path_buf.file_name().unwrap_or_default();
    let share_file = share_dir.join(file_name);
    
    // Use hard link for instant zero-copy share without exposing parent directory
    std::fs::hard_link(&path_buf, &share_file).or_else(|_| std::fs::copy(&path_buf, &share_file).map(|_| ())).map_err(|e| e.to_string())?;

    let cf_pin = get_cloudflared_pin();
    let cp_pin = get_copyparty_pin();
    
    let cf_bin = ensure_binary(cf_pin, &bin_dir).await.map_err(|e| e.to_string())?;
    let cp_bin = ensure_binary(cp_pin, &bin_dir).await.map_err(|e| e.to_string())?;
    
    // Generate random token for basic auth
    let token = format!("{:06}", rand::random::<u32>() % 1000000);
    
    let tunnel = WanTunnel::start(
        &cp_bin,
        &cf_bin,
        &share_dir,
        token.clone(),
        None,
    ).await.map_err(|e| format!("Failed to start WAN tunnel: {}", e))?;
    
    // Construct the direct download URL with basic auth parameters
    let url = format!("{}/{}?pw={}", tunnel.url, file_name.to_string_lossy(), token);
    
    let mut guard = state.wan_tunnel.lock().await;
    *guard = Some(tunnel);
    
    Ok(url)
}

#[tauri::command]
async fn download_wan_tunnel(url: String, state: tauri::State<'_, AppState>, app_handle: tauri::AppHandle) -> Result<(), String> {
    use tokio::io::AsyncWriteExt;
    use crate::protocol::TransferProgressEvent;
    
    let out_dir = state.output_dir.read().await.clone();
    
    // Parse URL to extract filename
    let parsed_url = reqwest::Url::parse(&url).map_err(|e| e.to_string())?;
    let path_segments = parsed_url.path_segments().ok_or("Invalid URL")?;
    let file_name = path_segments.last().unwrap_or("downloaded_file");
    
    let out_path = std::path::Path::new(&out_dir).join(file_name);
    
    let mut response = reqwest::get(&url).await.map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("Download failed: HTTP {}", response.status()));
    }
    
    let total_size = response.content_length().unwrap_or(0);
    let mut file = tokio::fs::File::create(&out_path).await.map_err(|e| e.to_string())?;
    
    let req_id = uuid::Uuid::new_v4().to_string();
    let mut downloaded = 0u64;
    let mut last_progress = std::time::Instant::now();
    let progress_interval = std::time::Duration::from_millis(200);
    
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        file.write_all(&chunk).await.map_err(|e| e.to_string())?;
        downloaded += chunk.len() as u64;
        
        if last_progress.elapsed() > progress_interval {
            let _ = app_handle.emit_all(
                "receive-progress",
                TransferProgressEvent {
                    request_id: req_id.clone(),
                    bytes_transferred: downloaded,
                    total: total_size,
                }
            );
            last_progress = std::time::Instant::now();
        }
    }
    
    let _ = app_handle.emit_all(
        "receive-progress",
        TransferProgressEvent {
            request_id: req_id,
            bytes_transferred: downloaded,
            total: total_size,
        }
    );
    
    Ok(())
}

/* ── Main ──────────────────────────────────────────────────── */

fn main() {
    tracing_subscriber::fmt().init();

    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("p2p");

    let resume_manager = Arc::new(
        TransferResumeManager::new(data_dir.join("resume"))
            .expect("Failed to init resume manager"),
    );

    // Load persistent identity
    let secret_key = identity::load_or_create_identity(&data_dir)
        .expect("Failed to load or create identity");
    let device_name = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "Unknown".to_string());

    let default_output = dirs::download_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .to_string_lossy()
        .to_string();

    let pending_requests = protocol::PendingRequests::new();
    let output_dir = Arc::new(RwLock::new(default_output));

    // We need the Tauri runtime for the async iroh setup
    tauri::Builder::default()
        .setup(move |app| {
            let app_handle = app.handle();
            let pending = pending_requests.clone();
            let out_dir = output_dir.clone();
            let sk = secret_key.clone();
            let d_name = device_name.clone();
            let d_dir = data_dir.clone();
            let rm = resume_manager.clone();

            // Spawn the iroh endpoint in the Tauri async runtime
            let endpoint_handle = tauri::async_runtime::block_on(async {
                // ── PERFORMANCE: Tune QUIC transport for maximum LAN throughput ──
                // Keep default CUBIC, but increase flow control windows drastically
                let transport = QuicTransportConfig::builder()
                    .stream_receive_window(VarInt::from_u32(64 * 1024 * 1024))  // 64 MB per stream
                    .receive_window(VarInt::from_u32(128 * 1024 * 1024))        // 128 MB total
                    .send_window(128 * 1024 * 1024)                              // 128 MB send
                    .build();

                let endpoint = Endpoint::builder(presets::N0)
                    .secret_key(sk.clone())
                    .transport_config(transport)
                    .bind()
                    .await
                    .expect("Failed to bind iroh endpoint");

                let device_id = endpoint.id().to_string();
                tracing::info!("Device ID: {}", device_id);

                let transfer_protocol = protocol::TransferProtocol {
                    pending: pending.clone(),
                    app_handle: app_handle.clone(),
                    output_dir: out_dir.clone(),
                };

                let router = Router::builder(endpoint.clone())
                    .accept(protocol::ALPN, transfer_protocol)
                    .spawn();

                (Arc::new(endpoint), device_id, router)
            });

            let (iroh_endpoint, device_id, router) = endpoint_handle;

            app.manage(AppState {
                engine: TransferEngine::new(4),
                resume_manager: rm,
                discovery: Arc::new(RwLock::new(None)),
                listener_handle: Arc::new(RwLock::new(None)),
                shutdown_flag: Arc::new(AtomicBool::new(false)),
                wan_tunnel: Arc::new(tokio::sync::Mutex::new(None)),
                data_dir: d_dir,
                iroh_endpoint,
                iroh_router: Arc::new(RwLock::new(Some(router))),
                device_id,
                device_name: d_name,
                pending_requests: pending,
                output_dir: out_dir,
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_peers,
            list_transfers,
            pause_transfer,
            resume_transfer,
            get_config,
            set_config,
            ping,
            send_file,
            start_listening,
            stop_listening,
            get_default_download_dir,
            get_device_id,
            get_device_name,
            add_contact,
            list_contacts,
            remove_contact,
            send_to_contact,
            respond_to_transfer,
            set_output_dir,
            start_wan_tunnel,
            download_wan_tunnel,
            get_pending_transfers,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
