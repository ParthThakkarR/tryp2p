#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod identity;
mod protocol;

use p2ptransfer_core::crypto::aead;
use p2ptransfer_core::crypto::ecdh::EcdhKeyExchange;
use p2ptransfer_core::network::tcp;
use p2ptransfer_core::p2p::discovery::DiscoveryService;
use p2ptransfer_core::transfer::engine::TransferEngine;
use p2ptransfer_core::transfer::resume::TransferResumeManager;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::net::{TcpListener, TcpStream};
use tauri::{Manager, SystemTray, SystemTrayMenu, CustomMenuItem, SystemTrayEvent};
use tokio::sync::RwLock;

// Iroh identity-based transport
use iroh::{Endpoint, endpoint::presets, protocol::Router};
use iroh::endpoint::{QuicTransportConfig, VarInt};
use once_cell::sync::Lazy;

#[cfg(target_os = "windows")]
fn register_context_menu(app_path: &std::path::Path) {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let app_path_str = app_path.to_string_lossy();
    
    // Register for Files
    if let Ok((key, _)) = hkcu.create_subkey(r"Software\Classes\*\shell\Send with P2P\command") {
        let command = format!("\"{}\" \"%1\"", app_path_str);
        let _ = key.set_value("", &command);
    }
    // Register for Directories
    if let Ok((key, _)) = hkcu.create_subkey(r"Software\Classes\Directory\shell\Send with P2P\command") {
        let command = format!("\"{}\" \"%1\"", app_path_str);
        let _ = key.set_value("", &command);
    }
}

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
    iroh_endpoint: Arc<tokio::sync::OnceCell<Endpoint>>,
    iroh_router: Arc<RwLock<Option<Router>>>,
    device_id: String,
    device_name: String,
    pending_requests: protocol::PendingRequests,
    output_dir: Arc<RwLock<String>>,
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
    online: Option<bool>,
    online_error: Option<String>,
}

/* â”€â”€ Existing LAN commands (preserved) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€ */

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
fn pause_transfer(request_id: String, app_handle: tauri::AppHandle, state: tauri::State<'_, AppState>) {
    let flag = protocol::TRANSFER_PAUSE_FLAGS
        .entry(request_id.clone())
        .or_insert_with(protocol::PauseFlag::new)
        .clone();
    flag.pause();

    if let Some(mut t_state) = protocol::ACTIVE_TRANSFERS.get_mut(&request_id) {
        t_state.status = "paused".to_string();
        let _ = state.resume_manager.pause_transfer(&request_id, t_state.bytes_transferred as i64);
    }

    let _ = app_handle.emit_all("transfer-paused", ());
}

#[tauri::command]
fn resume_transfer(request_id: String, app_handle: tauri::AppHandle, state: tauri::State<'_, AppState>) {
    if let Some(flag) = protocol::TRANSFER_PAUSE_FLAGS.get(&request_id) {
        flag.resume();
    }
    
    if let Some(mut t_state) = protocol::ACTIVE_TRANSFERS.get_mut(&request_id) {
        t_state.status = "transferring".to_string();
        let _ = state.resume_manager.update_transfer_status(&request_id, p2ptransfer_core::transfer::resume::TransferStatus::InProgress);
    }
    
    let _ = app_handle.emit_all("transfer-resumed", ());
}

#[tauri::command]
fn cancel_transfer(request_id: String, app_handle: tauri::AppHandle, state: tauri::State<'_, AppState>) {
    // Signal the pause/cancel flag so any in-flight transfer loop exits.
    if let Some(flag) = protocol::TRANSFER_PAUSE_FLAGS.get(&request_id) {
        flag.cancel();
    }

    // Delete the transfer record entirely — cancel means gone forever, not 'failed'.
    let _ = state.resume_manager.delete_transfer(&request_id);

    protocol::ACTIVE_TRANSFERS.remove(&request_id);
    protocol::TRANSFER_PAUSE_FLAGS.remove(&request_id);

    let _ = app_handle.emit_all("transfer-cancelled", ());
}

#[tauri::command]
fn delete_transfer(request_id: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let _ = state.resume_manager.delete_transfer(&request_id);
    protocol::ACTIVE_TRANSFERS.remove(&request_id);
    protocol::TRANSFER_PAUSE_FLAGS.remove(&request_id);
    Ok(())
}

/// Returns all request IDs currently awaiting user acceptance.
/// Used by the frontend on reconnect to re-display any pending popups.
#[tauri::command]
async fn get_pending_transfers(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, String> {
    Ok(state.pending_requests.pending_ids().await)
}

/// Returns all active transfers (in progress, connecting, etc.)
#[tauri::command]
async fn get_active_transfers() -> Result<Vec<protocol::ActiveTransferState>, String> {
    Ok(protocol::ACTIVE_TRANSFERS.iter().map(|entry| entry.value().clone()).collect())
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
        tcp::connect(peer_addr, 4 * 1024 * 1024),
    ).await;
    if let Ok(Ok(s)) = direct_result {
        return Ok(s);
    }

    Err(format!(
        "Cannot reach {peer_addr}.\n\n\
         Use the WAN mode (Send â†’ pick contact) for cross-network transfers.\n\
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

    let hostname = state.device_name.clone();
    let port = 9877;
    let config_port = 9876;
    let mut discovery = p2ptransfer_core::p2p::discovery::DiscoveryService::new(hostname, port, config_port)
        .await
        .map_err(|e| e.to_string())?;
    discovery.start().await.map_err(|e| e.to_string())?;
    *state.discovery.write().await = Some(discovery);

    let pending_reqs = state.pending_requests.clone();
    let shutdown = state.shutdown_flag.clone();
    let log_path = std::path::PathBuf::from(&output_dir).join("p2p_debug.log");
    let log_path2 = log_path.clone();
    let output_dir2 = output_dir.clone();
    let log_path_clone = log_path.clone();
    let app_handle_1 = app_handle.clone();
    let handle = tauri::async_runtime::spawn(async move {
        log_msg(&log_path_clone, "Listening for incoming transfers on port 9877");
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
                    let pending_1 = pending_reqs.clone();
                    tauri::async_runtime::spawn(async move {
                        let result = handle_incoming(stream, addr, out, app_h, pending_1).await;
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
    pending: protocol::PendingRequests,
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

    let req_id = uuid::Uuid::new_v4().to_string();
    let rx = pending.register(req_id.clone()).await;

    let _ = app_handle.emit_all(
        "transfer-incoming",
        protocol::IncomingTransferEvent {
            request_id: req_id.clone(),
            sender_name: addr.to_string(),
            sender_node_id: String::new(),
            file_name: metadata.file_name.clone(),
            file_size: metadata.file_size,
        },
    );

    open_receive_overlay(req_id.clone(), app_handle.clone());

    let answer = match tokio::time::timeout(std::time::Duration::from_secs(60), rx).await {
        Ok(Ok(true)) => true,
        _ => false,
    };

    if !answer {
        send_err(&mut stream, "timeout");
        return Err("Transfer rejected".into());
    }

    // Check for existing partial file to resume
    let output_path = output_dir.join(&metadata.file_name);
    if let Some(parent) = output_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let existing_len = if output_path.exists() {
        std::fs::metadata(&output_path).map(|m| m.len()).unwrap_or(0)
    } else {
        0
    };

    let (resume_offset, is_resume) = if existing_len > 0 && existing_len < metadata.file_size {
        // Align resume offset to LAN_BLOCK_SIZE boundary
        let aligned = (existing_len / LAN_BLOCK_SIZE as u64) * LAN_BLOCK_SIZE as u64;
        (aligned, true)
    } else {
        (0u64, false)
    };

    // Send ACCEPT response (with offset if resuming)
    let mut accept_msg = vec![0x00u8]; // TAG_METADATA
    if resume_offset > 0 {
        accept_msg.extend_from_slice(format!("ACCEPT:{resume_offset}").as_bytes());
    } else {
        accept_msg.extend_from_slice(b"ACCEPT");
    }
    tcp::send_message(&mut stream, &accept_msg).await.map_err(|e| {
        let msg = format!("Failed to send ACCEPT to {addr}: {e}");
        eprintln!("{msg}");
        msg
    })?;

    // ── Dedicated spawn_blocking Decrypt + Disk Writer Task ──────────────
    // Decryption (ChaCha20-Poly1305 on 4 MB blocks) is CPU-bound.
    // By combining decrypt + disk write in one blocking task, the async loop
    // is free to read the next network message while decryption runs.
    // Channel carries: (block_index, encrypted_payload_from_byte_6_onward)
    let (disk_tx, mut disk_rx) = tokio::sync::mpsc::channel::<(u64, Vec<u8>)>(32);
    let output_path_writer = output_path.clone();
    let enc_key_writer = enc_key;
    let nonce_prefix_writer = nonce_prefix;

    let writer_task = tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        use std::io::Write;
        let file = if is_resume {
            std::fs::OpenOptions::new()
                .write(true)
                .append(true)
                .open(&output_path_writer)?
        } else {
            std::fs::File::create(&output_path_writer)?
        };
        let mut writer = std::io::BufWriter::with_capacity(4 * 1024 * 1024, file);
        while let Some((blk_idx, encrypted_data)) = disk_rx.blocking_recv() {
            let nonce = p2ptransfer_core::crypto::aead::build_nonce(&nonce_prefix_writer, blk_idx);
            let decrypted = p2ptransfer_core::crypto::aead::decrypt(&enc_key_writer, &nonce, &encrypted_data)
                .map_err(|e| anyhow::anyhow!("Decryption failed for block {blk_idx}: {e}"))?;
            writer.write_all(&decrypted)?;
        }
        writer.flush()?;
        Ok(())
    });

    let start_time = std::time::Instant::now();
    let mut block_index: u64 = resume_offset / LAN_BLOCK_SIZE as u64;
    let mut received_hash = String::new();
    let mut bytes_received: u64 = resume_offset;
    let mut speed_window_start = std::time::Instant::now();
    let mut bytes_in_window: u64 = 0;
    let mut current_speed: f64 = 0.0;
    let mut last_progress = std::time::Instant::now();
    let progress_interval = std::time::Duration::from_millis(50);

    protocol::ACTIVE_TRANSFERS.insert(req_id.clone(), protocol::ActiveTransferState {
        request_id: req_id.clone(),
        role: "receiver".to_string(),
        status: "transferring".to_string(),
        file_name: metadata.file_name.clone(),
        total_bytes: metadata.file_size,
        bytes_transferred: resume_offset,
    });

    // Async loop: reads framed messages and dispatches encrypted payloads.
    // Decryption happens in the blocking writer task above — zero CPU stalls here.
    loop {
        let msg_data = tcp::receive_message(&mut stream).await.map_err(|e| {
            protocol::ACTIVE_TRANSFERS.remove(&req_id);
            format!("Network read error from {addr}: {e}")
        })?;

        if msg_data.is_empty() {
            protocol::ACTIVE_TRANSFERS.remove(&req_id);
            send_err(&mut stream, "Empty message");
            return Err("Empty message received".into());
        }

        match msg_data[0] {
            0x01 => {
                if msg_data.len() < 6 {
                    protocol::ACTIVE_TRANSFERS.remove(&req_id);
                    send_err(&mut stream, "Malformed chunk");
                    return Err("Malformed chunk frame".into());
                }

                // Estimate plaintext size from ciphertext (subtract 16-byte Poly1305 tag)
                let ciphertext_len = msg_data.len() - 6;
                let plaintext_len = ciphertext_len.saturating_sub(16);

                // Forward encrypted payload to the decrypt+write task
                if disk_tx.send((block_index, msg_data[6..].to_vec())).await.is_err() {
                    protocol::ACTIVE_TRANSFERS.remove(&req_id);
                    send_err(&mut stream, "Disk writer task died");
                    return Err("Disk writer task died".into());
                }

                bytes_received += plaintext_len as u64;
                bytes_in_window += plaintext_len as u64;

                let window_elapsed = speed_window_start.elapsed().as_secs_f64();
                if window_elapsed >= 0.5 {
                    current_speed = (bytes_in_window as f64) / window_elapsed;
                    bytes_in_window = 0;
                    speed_window_start = std::time::Instant::now();
                }

                if last_progress.elapsed() > progress_interval {
                    if let Some(mut state) = protocol::ACTIVE_TRANSFERS.get_mut(&req_id) {
                        state.bytes_transferred = bytes_received;
                    }
                    let _ = app_handle.emit_all(
                        "transfer-progress",
                        protocol::TransferProgressEvent {
                            request_id: req_id.clone(),
                            bytes_transferred: bytes_received,
                            total: metadata.file_size,
                            speed_bytes_per_sec: current_speed,
                        },
                    );
                    last_progress = std::time::Instant::now();
                }

                block_index += 1;
            }
            0x03 => {
                received_hash = String::from_utf8_lossy(&msg_data[1..]).to_string();
                break;
            }
            0x04 => {
                protocol::ACTIVE_TRANSFERS.remove(&req_id);
                let err = String::from_utf8_lossy(&msg_data[1..]).to_string();
                return Err(format!("Sender error: {err}"));
            }
            tag => {
                protocol::ACTIVE_TRANSFERS.remove(&req_id);
                send_err(&mut stream, &format!("Unexpected tag: {tag:#04x}"));
                return Err(format!("Unexpected message tag: {tag:#04x}"));
            }
        }
    }

    // Drop disk_tx to signal completion to writer task
    drop(disk_tx);

    // Wait for writer task to finish flushing to disk
    match writer_task.await {
        Ok(Ok(_)) => {},
        _ => {
            protocol::ACTIVE_TRANSFERS.remove(&req_id);
            return Err("Disk flush failed".into());
        }
    }

    let engine = TransferEngine::new(4);
    let valid = engine
        .verify_checksum(&output_path, &metadata.checksum)
        .await
        .map_err(|e| {
            protocol::ACTIVE_TRANSFERS.remove(&req_id);
            e.to_string()
        })?;

    if valid {
        let mut complete = vec![0x03u8];
        complete.extend_from_slice(received_hash.as_bytes());
        tcp::send_message(&mut stream, &complete).await.map_err(|e| {
            let msg = format!("Failed to send COMPLETE ACK to {addr}: {e}");
            eprintln!("{msg}");
            msg
        })?;
        
        let elapsed = start_time.elapsed().as_secs_f64();
        let _ = app_handle.emit_all(
            "transfer-complete",
            protocol::TransferCompleteEvent {
                request_id: req_id.clone(),
                file_path: output_path.to_string_lossy().to_string(),
                blake3_hash: blake3::Hash::from(metadata.checksum).to_hex().to_string(),
                elapsed_secs: elapsed,
            },
        );
        protocol::ACTIVE_TRANSFERS.remove(&req_id);
        eprintln!("Transfer complete from {addr}: {} ({block_index} blocks)", metadata.file_name);
    } else {
        protocol::ACTIVE_TRANSFERS.remove(&req_id);
        let err_msg = format!("Checksum mismatch for {} from {addr}", metadata.file_name);
        eprintln!("{err_msg}");
        let mut error_resp = vec![0x04u8];
        error_resp.extend_from_slice(b"Checksum mismatch");
        let _ = tcp::send_message(&mut stream, &error_resp).await;
        return Err(err_msg);
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

/* â”€â”€ Identity-based commands â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€ */

/// Returns this device's permanent ID (iroh NodeId).
#[tauri::command]
async fn get_device_id(state: tauri::State<'_, AppState>) -> Result<String, String> {
    Ok(state.device_id.clone())
}

/// Returns a short 8-char hex device ID formatted as XXXX-XXXX.
/// This is the user-facing "key" shown in the UI — same format as the mobile app.
#[tauri::command]
async fn get_short_device_id(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let full = &state.device_id;
    let hex8: String = full.chars().take(8).collect::<String>().to_uppercase();
    if hex8.len() == 8 {
        Ok(format!("{}-{}", &hex8[..4], &hex8[4..]))
    } else {
        Ok(full.clone())
    }
}

/// Returns the device display name.
#[tauri::command]
async fn get_device_name(state: tauri::State<'_, AppState>) -> Result<String, String> {
    Ok(state.device_name.clone())
}

/* â”€â”€ Contact management commands â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€ */

#[tauri::command]
async fn add_contact(
    name: String,
    node_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut ip = String::new();
    let mut port: u16 = 0;
    
    // Attempt to resolve IP using LAN discovery by matching device name
    let discovery_lock = state.discovery.read().await;
    if let Some(discovery) = discovery_lock.as_ref() {
        let peers = discovery.get_peers().await;
        if let Some(peer) = peers.iter().find(|p| p.device_name == name) {
            ip = peer.socket_addr.ip().to_string();
            port = peer.socket_addr.port();
        }
    }
    drop(discovery_lock);

    if let Ok(pk) = node_id.parse::<iroh::PublicKey>() {
        let mut target_node = iroh::EndpointAddr::new(pk);
        if !ip.is_empty() && port > 0 {
            let addr_str = format!("{}:{}", ip, port);
            if let Ok(socket_addr) = addr_str.parse::<std::net::SocketAddr>() {
                target_node = target_node.with_ip_addr(socket_addr);
            }
        }
        
        if let Some(endpoint) = state.iroh_endpoint.get().cloned() {
            // Pre-connect to cache the address in iroh. 4s to allow relay discovery + NAT traversal.
            if let Ok(Ok(_conn)) = tokio::time::timeout(
                std::time::Duration::from_millis(4000),
                endpoint.connect(target_node, protocol::ALPN)
            ).await {
                // Connection successful! Address will be cached by Iroh.
            }
        }
    }
    
    state
        .resume_manager
        .upsert_contact(&name, &node_id, &ip, port)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_contacts(state: tauri::State<'_, AppState>) -> Result<Vec<ContactEntry>, String> {
    let contacts = state
        .resume_manager
        .list_contacts()
        .map_err(|e| e.to_string())?;

    let endpoint_opt = state.iroh_endpoint.get().cloned();

    let discovery_guard = state.discovery.read().await;
    let lan_peers = if let Some(d) = discovery_guard.as_ref() {
        d.get_peers().await
    } else {
        Vec::new()
    };
    drop(discovery_guard);

    let mut tasks = Vec::new();

    for c in contacts {
        let ep_opt = endpoint_opt.clone();
        let node_id_str = c.peer_id.clone();
        let name = c.name.clone();
        let mut ip = c.last_known_ip.clone();
        let mut port = c.last_known_port;
        let resume_mgr = state.resume_manager.clone();

        let found_lan_peer = lan_peers.iter().find(|p| {
            p.device_name == name || (p.node_id.is_some() && p.node_id.as_ref() == Some(&node_id_str))
        });
        let is_on_lan = found_lan_peer.is_some();
        if let Some(peer) = found_lan_peer {
            ip = peer.socket_addr.ip().to_string();
            port = peer.socket_addr.port();
            let _ = resume_mgr.upsert_contact(&name, &node_id_str, &ip, port);
        }

        tasks.push(tokio::spawn(async move {
            let mut online = false;
            let mut online_error = None;

            if is_on_lan {
                online = true;
            } else if let Some(ep) = ep_opt {
                if let Ok(node_id) = node_id_str.parse::<iroh::PublicKey>() {
                    let mut target_node = iroh::EndpointAddr::new(node_id);
                    if !ip.is_empty() && port > 0 {
                        let addr_str = format!("{}:{}", ip, port);
                        if let Ok(socket_addr) = addr_str.parse::<std::net::SocketAddr>() {
                            target_node = target_node.with_ip_addr(socket_addr);
                        }
                    }

                    match tokio::time::timeout(
                        std::time::Duration::from_millis(3000),
                        ep.connect(target_node, protocol::ALPN)
                    ).await {
                        Ok(Ok(_)) => { online = true; },
                        Ok(Err(e)) => {
                            let msg = format!("{:#}", e);
                            if msg.contains("No addressing information") {
                                online_error = Some("Peer address not found. Share your device ID again.".into());
                            }
                        }
                        Err(_) => {},
                    }
                } else {
                    online_error = Some("Invalid node ID".into());
                }
            }

            ContactEntry {
                name,
                node_id: node_id_str,
                online: Some(online),
                online_error,
            }
        }));
    }

    let mut results = Vec::new();
    for t in tasks {
        if let Ok(entry) = t.await {
            results.push(entry);
        }
    }
    
    Ok(results)
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

/* â”€â”€ Identity-based transfer commands â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€ */

fn is_same_lan(peer_ip: &str) -> bool {
    // Part 14: Direct connection > relay
    if let Ok(ip) = local_ip_address::local_ip() {
        let local = ip.to_string();
        match (peer_ip, local.as_str()) {
            (p, l) if p.starts_with("192.168.") && l.starts_with("192.168.") => true,
            (p, l) if p.starts_with("10.") && l.starts_with("10.") => true,
            (p, l) if p.starts_with("172.") && l.starts_with("172.") => {
                let p_ip = p.parse::<std::net::IpAddr>();
                let l_ip = l.parse::<std::net::IpAddr>();
                if let (Ok(std::net::IpAddr::V4(p4)), Ok(std::net::IpAddr::V4(l4))) = (p_ip, l_ip) {
                    p4.octets()[1] >= 16 && p4.octets()[1] <= 31 &&
                    l4.octets()[1] >= 16 && l4.octets()[1] <= 31
                } else {
                    false
                }
            },
            _ => false
        }
    } else { false }
}

/// Struct holding the path of a temp archive for directory sending.
struct TempArchive {
    path: std::path::PathBuf,
}

/// Walk a directory recursively and pack its contents into a single temp tar archive file.
fn archive_directory_to_temp(dir: &std::path::Path) -> Result<TempArchive, String> {
    let folder_name = dir.file_name().unwrap_or_default().to_string_lossy().to_string();
    let temp_dir = std::env::temp_dir().join("p2ptransfer");
    let _ = std::fs::create_dir_all(&temp_dir);
    let archive_path = temp_dir.join(format!("{folder_name}.tar"));

    let archive_file = std::fs::File::create(&archive_path)
        .map_err(|e| format!("Failed to create temp tar file: {e}"))?;
    let mut builder = tar::Builder::new(archive_file);

    builder
        .append_dir_all(&folder_name, dir)
        .map_err(|e| format!("Failed to pack folder into tar archive: {e}"))?;

    builder
        .finish()
        .map_err(|e| format!("Failed to finish tar archive: {e}"))?;

    Ok(TempArchive { path: archive_path })
}

/// If the path is a directory, it is automatically archived into a single .p2parchive file
/// before sending — no recursive per-file popups.
/// If receiver is offline, stays in the queue retrying until sender cancels or receiver comes online.
#[tauri::command]
async fn send_to_contact(
    request_id: String,
    path: String,
    contact_name: String,
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    let contact = state
        .resume_manager
        .get_contact(&contact_name)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Contact '{}' not found", contact_name))?;

    let original_path = std::path::PathBuf::from(&path);
    if !original_path.exists() {
        return Err(format!("File not found: {}", path));
    }

    // If the path is a directory, archive it into a temp file so it sends as one unit.
    let is_dir = original_path.is_dir();
    let (send_path, temp_cleanup) = if is_dir {
        let archive = archive_directory_to_temp(&original_path).map_err(|e| e.to_string())?;
        (archive.path.clone(), Some(archive))
    } else {
        (original_path.clone(), None)
    };

    let file_name = send_path.file_name().unwrap_or_default().to_string_lossy().to_string();
    let file_size = std::fs::metadata(&send_path).map(|m| m.len()).unwrap_or(0);
    let send_path_str = send_path.to_string_lossy().to_string();

    // Register pause/cancel flag & active transfer state for offline queue
    let flag = protocol::PauseFlag::new();
    protocol::TRANSFER_PAUSE_FLAGS.insert(request_id.clone(), flag.clone());

    protocol::ACTIVE_TRANSFERS.insert(request_id.clone(), protocol::ActiveTransferState {
        request_id: request_id.clone(),
        role: "sender".to_string(),
        status: "queued".to_string(),
        file_name,
        total_bytes: file_size,
        bytes_transferred: 0,
    });

    let _ = app_handle.emit_all("send-status", protocol::SendStatusEvent {
        request_id: request_id.clone(),
        status: "queued".to_string(),
    });

    let result: Result<String, String> = loop {
        // Check if user cancelled while queued
        if flag.wait_if_paused().await {
            protocol::ACTIVE_TRANSFERS.remove(&request_id);
            protocol::TRANSFER_PAUSE_FLAGS.remove(&request_id);
            let _ = app_handle.emit_all("transfer-cancelled", ());
            break Err("REJECTED".to_string());
        }

        // Fetch latest contact info (in case LAN discovery refreshed IP/port)
        let current_contact = state
            .resume_manager
            .get_contact(&contact_name)
            .ok()
            .flatten()
            .unwrap_or_else(|| contact.clone());

        // 1. Attempt direct TCP if on LAN
        if is_same_lan(&current_contact.last_known_ip) {
            match send_file_direct_tcp(request_id.clone(), send_path_str.clone(), &current_contact, &state, &app_handle).await {
                Ok(hash) => {
                    protocol::ACTIVE_TRANSFERS.remove(&request_id);
                    protocol::TRANSFER_PAUSE_FLAGS.remove(&request_id);
                    break Ok(hash);
                }
                Err(e) if e.contains("rejected") || e == "REJECTED" => {
                    protocol::ACTIVE_TRANSFERS.remove(&request_id);
                    protocol::TRANSFER_PAUSE_FLAGS.remove(&request_id);
                    break Err(e);
                }
                Err(_) => {
                    // Receiver offline or port unreachable right now, stay queued
                }
            }
        }

        // 2. Attempt Iroh QUIC if endpoint ready
        if let Some(endpoint) = state.iroh_endpoint.get().cloned() {
            if let Ok(node_id) = current_contact.peer_id.parse::<iroh::PublicKey>() {
                let mut target_node = iroh::EndpointAddr::new(node_id);
                if !current_contact.last_known_ip.is_empty() && current_contact.last_known_port > 0 {
                    if let Ok(sa) = format!("{}:{}", current_contact.last_known_ip, current_contact.last_known_port).parse::<std::net::SocketAddr>() {
                        target_node = target_node.with_ip_addr(sa);
                    }
                }

                match protocol::send_file_to_peer(
                    request_id.clone(),
                    &endpoint,
                    target_node,
                    &send_path,
                    &state.device_name,
                    &app_handle,
                ).await {
                    Ok(hash) => {
                        protocol::ACTIVE_TRANSFERS.remove(&request_id);
                        protocol::TRANSFER_PAUSE_FLAGS.remove(&request_id);
                        break Ok(hash);
                    }
                    Err(e) if e.to_string().contains("REJECTED") || e.to_string().contains("cancelled") => {
                        protocol::ACTIVE_TRANSFERS.remove(&request_id);
                        protocol::TRANSFER_PAUSE_FLAGS.remove(&request_id);
                        break Err(e.to_string());
                    }
                    Err(_) => {
                        // Offline or timeout, stay queued and retry
                    }
                }
            }
        }

        // Still offline: reset status back to "queued" and wait 3 seconds before next retry
        if let Some(mut st) = protocol::ACTIVE_TRANSFERS.get_mut(&request_id) {
            st.status = "queued".to_string();
        }
        let _ = app_handle.emit_all("send-status", protocol::SendStatusEvent {
            request_id: request_id.clone(),
            status: "queued".to_string(),
        });

        // Cancel-interruptible 3s sleep
        tokio::select! {
            _ = flag.wait_for_cancel() => {
                protocol::ACTIVE_TRANSFERS.remove(&request_id);
                protocol::TRANSFER_PAUSE_FLAGS.remove(&request_id);
                let _ = app_handle.emit_all("transfer-cancelled", ());
                break Err("Transfer cancelled by user".to_string());
            }
            _ = tokio::time::sleep(std::time::Duration::from_secs(3)) => {}
        }
    };

    // Clean up temp archive if one was created
    if let Some(cleanup) = temp_cleanup {
        let _ = std::fs::remove_file(&cleanup.path);
    }

    result
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

pub static PENDING_SEND_FILES: Lazy<std::sync::RwLock<Vec<String>>> = Lazy::new(|| std::sync::RwLock::new(Vec::new()));

#[tauri::command]
fn get_pending_send_files() -> Vec<String> {
    let files = PENDING_SEND_FILES.read().unwrap();
    files.clone()
}

#[tauri::command]
fn clear_pending_send_files() {
    let mut files = PENDING_SEND_FILES.write().unwrap();
    files.clear();
}

pub fn open_send_overlay(file_path: String, app_handle: tauri::AppHandle) {
    if !file_path.is_empty() {
        let mut files = PENDING_SEND_FILES.write().unwrap();
        if !files.contains(&file_path) {
            files.push(file_path.clone());
        }
    }

    if let Some(win) = app_handle.get_window("send-overlay") {
        let _ = app_handle.emit_all("add-file-to-send", serde_json::json!({ "path": file_path }));
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
        return;
    }

    let url_path = format!("index.html?overlay=send&file={}", urlencoding::encode(&file_path));
    if let Err(e) = tauri::WindowBuilder::new(
        &app_handle,
        "send-overlay",
        tauri::WindowUrl::App(url_path.into())
    )
    .title("Send Files")
    .inner_size(680.0, 460.0)
    .min_inner_size(580.0, 380.0)
    .always_on_top(false)
    .decorations(true)
    .transparent(false)
    .center()
    .build() {
        tracing::warn!("Failed to open send overlay: {}", e);
    }
}

pub fn open_receive_overlay(request_id: String, app_handle: tauri::AppHandle) {
    let window_label = format!("recv-overlay-{}", request_id);
    if let Some(win) = app_handle.get_window(&window_label) {
        let _ = win.set_always_on_top(true);
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
        return;
    }

    let url_path = format!("index.html?overlay=receive&id={}", request_id);
    match tauri::WindowBuilder::new(
        &app_handle,
        &window_label,
        tauri::WindowUrl::App(url_path.into())
    )
    .title("Incoming Transfer")
    .inner_size(420.0, 480.0)
    .min_inner_size(350.0, 400.0)
    .always_on_top(true)
    .decorations(true)
    .transparent(false)
    .center()
    .build() {
        Ok(win) => {
            let _ = win.set_always_on_top(true);
            let _ = win.unminimize();
            let _ = win.show();
            let _ = win.set_focus();
        }
        Err(e) => {
            tracing::warn!("Failed to open receive overlay: {}", e);
        }
    }
}

#[derive(serde::Serialize)]
struct FileMetaInfo {
    path: String,
    name: String,
    size: u64,
    is_dir: bool,
}

#[tauri::command]
fn get_files_metadata(paths: Vec<String>) -> Vec<FileMetaInfo> {
    let mut result = Vec::new();
    for p in paths {
        let path_buf = std::path::PathBuf::from(&p);
        let name = path_buf
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| p.clone());

        let is_dir = path_buf.is_dir();
        let size = if is_dir {
            get_dir_size(&path_buf).unwrap_or(0)
        } else if let Ok(meta) = std::fs::metadata(&path_buf) {
            meta.len()
        } else {
            0
        };

        result.push(FileMetaInfo { path: p, name, size, is_dir });
    }
    result
}

fn get_dir_size(path: &std::path::Path) -> std::io::Result<u64> {
    let mut total = 0;
    if path.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let meta = entry.metadata()?;
            if meta.is_dir() {
                total += get_dir_size(&entry.path())?;
            } else {
                total += meta.len();
            }
        }
    }
    Ok(total)
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


/* â”€â”€ Main â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€ */

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
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            let file_args: Vec<String> = argv.iter().skip(1).filter(|a| !a.starts_with('-')).cloned().collect();
            if !file_args.is_empty() {
                for file_path in file_args {
                    open_send_overlay(file_path, app.clone());
                }
            } else {
                if let Some(window) = app.get_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        }))
        .system_tray(
            SystemTray::new().with_menu(
                SystemTrayMenu::new()
                    .add_item(CustomMenuItem::new("show".to_string(), "Show P2P Transfer"))
                    .add_item(CustomMenuItem::new("quit".to_string(), "Quit"))
            )
        )
        .on_system_tray_event(|app, event| match event {
            SystemTrayEvent::MenuItemClick { id, .. } => {
                match id.as_str() {
                    "quit" => { std::process::exit(0); }
                    "show" => {
                        if let Some(window) = app.get_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        })
        .on_window_event(|event| match event.event() {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                // Only intercept close for the persistent background window.
                // Overlay windows ("send-overlay-*", "recv-overlay-*") must be
                // allowed to close and be destroyed so they can be recreated
                // fresh for the next transfer with the correct URL / file path.
                if event.window().label() == "main" {
                    let _ = event.window().hide();
                    api.prevent_close();
                }
            }
            _ => {}
        })
        .setup(move |app| {
            #[cfg(target_os = "windows")]
            {
                if let Ok(exe_path) = std::env::current_exe() {
                    register_context_menu(&exe_path);
                }
            }
            
            // Check initial launch args
            let args: Vec<String> = std::env::args().collect();
            let file_args: Vec<String> = args.into_iter().skip(1).filter(|a| !a.starts_with('-')).collect();
            if !file_args.is_empty() {
                let app_handle = app.handle();
                for file_path in file_args {
                    open_send_overlay(file_path, app_handle.clone());
                }
            } else {
                if let Some(window) = app.get_window("main") {
                    let _ = window.hide();
                }
            }
            let app_handle = app.handle();
            let pending = pending_requests.clone();
            let out_dir = output_dir.clone();
            let sk = secret_key.clone();
            let d_name = device_name.clone();
            let d_dir = data_dir.clone();
            let rm = resume_manager.clone();

            // Spawn the iroh endpoint binding in the background so we don't block Tauri setup
            let device_id = sk.public().to_string();
            let iroh_endpoint = Arc::new(tokio::sync::OnceCell::new());
            let iroh_router = Arc::new(RwLock::new(None));

            let endpoint_clone = iroh_endpoint.clone();
            let router_clone = iroh_router.clone();
            let sk_clone = sk.clone();
            let pending_clone = pending.clone();
            let app_handle_clone = app.handle();
            let out_dir_clone = out_dir.clone();

            tauri::async_runtime::spawn(async move {
                // Use 1280-byte initial MTU: safe for relay-encapsulated UDP datagrams
                // (relay adds overhead, so 1500 causes black-holing on relay paths).
                // Path MTU discovery will raise this for direct connections automatically.
                //
                // Window sizing: 256 MB recv / 512 MB send handles high-latency WAN paths.
                // Example: 100 Mbps link at 100ms RTT â†’ BDP = 1.25 MB. 256 MB is >200Ã— that,
                // so QUIC flow control is never the bottleneck even on intercontinental links.
                let transport = QuicTransportConfig::builder()
                    .stream_receive_window(VarInt::from_u32(256 * 1024 * 1024))
                    .receive_window(VarInt::from_u32(512 * 1024 * 1024))
                    .send_window(512 * 1024 * 1024)
                    .initial_mtu(1280)
                    .build();

                // presets::N0 already enables the n0 relay servers via default_relay_mode().
                // Do NOT call .relay_mode(RelayMode::Disabled) â€” that breaks WAN connectivity.
                match Endpoint::builder(presets::N0)
                    .secret_key(sk_clone)
                    .transport_config(transport)
                    .bind()
                    .await
                {
                    Ok(endpoint) => {
                        tracing::info!("Iroh endpoint bound. Node ID: {}", endpoint.id());
                        let transfer_protocol = protocol::TransferProtocol {
                            pending: pending_clone,
                            app_handle: app_handle_clone,
                            output_dir: out_dir_clone,
                        };

                        let router = Router::builder(endpoint.clone())
                            .accept(protocol::ALPN, transfer_protocol)
                            .spawn();

                        let _ = endpoint_clone.set(endpoint);
                        *router_clone.write().await = Some(router);
                        tracing::info!("Iroh endpoint bound successfully in background");
                    }
                    Err(e) => {
                        tracing::error!("Failed to bind iroh endpoint: {e}");
                    }
                }
            });

            let discovery_lock = Arc::new(RwLock::new(None));
            let discovery_clone = discovery_lock.clone();
            let d_name_disc = d_name.clone();
            let d_id_disc = device_id.clone();

            tauri::async_runtime::spawn(async move {
                if let Ok(mut discovery) = p2ptransfer_core::p2p::discovery::DiscoveryService::new_with_node_id(d_name_disc, 9877, 9876, Some(d_id_disc)).await {
                    if let Ok(_) = discovery.start().await {
                        *discovery_clone.write().await = Some(discovery);
                        tracing::info!("LAN Discovery service started successfully");
                    }
                }
            });

            let pending_requests_tcp = pending.clone();
            let app_handle_tcp = app.handle();
            let output_dir_tcp = out_dir.clone();

            tauri::async_runtime::spawn(async move {
                if let Ok(listener) = TcpListener::bind("0.0.0.0:9877").await {
                    tracing::info!("LAN direct TCP listener bound on port 9877");
                    loop {
                        if let Ok((stream, addr)) = listener.accept().await {
                            let out = std::path::PathBuf::from(output_dir_tcp.read().await.clone());
                            let app_h = app_handle_tcp.clone();
                            let pending_reqs = pending_requests_tcp.clone();
                            tauri::async_runtime::spawn(async move {
                                let _ = handle_incoming(stream, addr, out, app_h, pending_reqs).await;
                            });
                        }
                    }
                }
            });

            app.manage(AppState {
                engine: TransferEngine::new(4),
                resume_manager: rm,
                discovery: discovery_lock,
                listener_handle: Arc::new(RwLock::new(None)),
                shutdown_flag: Arc::new(AtomicBool::new(false)),
                data_dir: d_dir,
                iroh_endpoint,
                iroh_router,
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
            cancel_transfer,
            get_config,
            set_config,
            ping,
            send_file,
            start_listening,
            stop_listening,
            get_default_download_dir,
            get_device_id,
            get_short_device_id,
            get_device_name,
            add_contact,
            list_contacts,
            remove_contact,
            send_to_contact,
            respond_to_transfer,
            set_output_dir,
            get_pending_transfers,
            get_active_transfers,
            get_pending_send_files,
            clear_pending_send_files,
            get_files_metadata,
            delete_transfer,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

const TAG_METADATA: u8 = 0x00;
// TAG_CHUNK/ACK are kept for potential future use but the streaming LAN protocol
// no longer uses per-chunk ACKs â€” only a final COMPLETE handshake.
#[allow(dead_code)]
const TAG_CHUNK: u8 = 0x01;
#[allow(dead_code)]
const TAG_CHUNK_ACK: u8 = 0x02;
const TAG_COMPLETE: u8 = 0x03;
const TAG_ERROR: u8 = 0x04;
const TAG_CLIENT_HELLO: u8 = 0x05;
const TAG_SERVER_HELLO: u8 = 0x06;

/// Block size for the streaming LAN TCP path.
/// 4 MB: optimized for Multi-Gigabit network interfaces and high-speed Wi-Fi 6/7.
const LAN_BLOCK_SIZE: usize = 4 * 1024 * 1024;

/// Maximum payload for the streaming receive buffer (64 MB per message).
#[allow(dead_code)] // reserved for future use
const LAN_MAX_MSG: u64 = 64 * 1024 * 1024;

async fn send_file_direct_tcp(
    request_id: String,
    path: String,
    contact: &p2ptransfer_core::transfer::resume::Contact,
    _state: &tauri::State<'_, AppState>,
    app_handle: &tauri::AppHandle,
) -> Result<String, String> {
    let _ = app_handle.emit_all("send-status", protocol::SendStatusEvent {
        request_id: request_id.clone(),
        status: "connecting".to_string(),
    });

    let addr_str = format!("{}:{}", contact.last_known_ip, contact.last_known_port);
    let peer_addr = addr_str.parse::<std::net::SocketAddr>().map_err(|e| e.to_string())?;

    // Connect with 32 MB socket buffers for multi-gigabit throughput.
    let direct_result = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        p2ptransfer_core::network::tcp::connect(peer_addr, 32 * 1024 * 1024),
    ).await;

    let mut stream = match direct_result {
        Ok(Ok(s)) => s,
        _ => {
            let msg = format!(
                "Failed to connect to {peer_addr}.\n\nCheck that the receiver is listening on port {} at that IP address.",
                peer_addr.port()
            );
            return Err(msg);
        }
    };

    let _ = app_handle.emit_all("send-status", protocol::SendStatusEvent {
        request_id: request_id.clone(),
        status: "accepted".to_string(),
    });

    // â”€â”€ ECDH key exchange â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
    let kx = p2ptransfer_core::crypto::ecdh::EcdhKeyExchange::new();
    let client_pub = kx.public_key_bytes();
    let mut hello_msg = vec![TAG_CLIENT_HELLO];
    hello_msg.extend_from_slice(&client_pub);
    p2ptransfer_core::network::tcp::send_message(&mut stream, &hello_msg)
        .await.map_err(|e| e.to_string())?;

    let resp = p2ptransfer_core::network::tcp::receive_message(&mut stream)
        .await.map_err(|e| e.to_string())?;
    if resp.is_empty() || resp[0] != TAG_SERVER_HELLO {
        return Err("Expected SERVER_HELLO".to_string());
    }
    let server_pub_bytes: [u8; 32] = resp[1..]
        .try_into().map_err(|_| "Invalid key length".to_string())?;
    let shared_secret = kx
        .derive_shared_secret(&server_pub_bytes).map_err(|e| e.to_string())?;
    let enc_key = p2ptransfer_core::crypto::aead::derive_encryption_key(
        &shared_secret, b"P2PTRANSFER_SALT_v1", b"p2ptransfer-v1-encryption"
    ).map_err(|e| e.to_string())?;
    let nonce_prefix = p2ptransfer_core::crypto::aead::generate_nonce_prefix();

    // ── File metadata ────────────────────────────────────────────────────────────
    let path_buf = std::path::PathBuf::from(&path);
    let engine = std::sync::Arc::new(TransferEngine::new(4));
    let mut metadata = engine
        .create_metadata(&path_buf, LAN_BLOCK_SIZE)
        .await.map_err(|e| e.to_string())?;
    metadata.nonce_prefix = nonce_prefix;

    let meta_json = serde_json::to_vec(&metadata).map_err(|e| e.to_string())?;
    let mut meta_msg = vec![TAG_METADATA];
    meta_msg.extend_from_slice(&meta_json);
    p2ptransfer_core::network::tcp::send_message(&mut stream, &meta_msg)
        .await.map_err(|e| e.to_string())?;

    let resp2 = p2ptransfer_core::network::tcp::receive_message(&mut stream)
        .await.map_err(|e| e.to_string())?;
    if resp2.is_empty() || resp2[0] != TAG_METADATA {
        return Err("Peer rejected transfer".to_string());
    }

    let resp_str = String::from_utf8_lossy(&resp2[1..]);
    let start_offset: u64 = if resp_str.starts_with("ACCEPT:") {
        resp_str[7..].parse().unwrap_or(0)
    } else if resp_str == "ACCEPT" {
        0
    } else {
        return Err("Peer rejected transfer".to_string());
    };

    let _ = app_handle.emit_all("send-status", protocol::SendStatusEvent {
        request_id: request_id.clone(),
        status: "transferring".to_string(),
    });

    // ── Dedicated spawn_blocking File Reader + Encrypt Task ─────────────
    // Encryption (ChaCha20-Poly1305 on 4 MB blocks) is CPU-bound work.
    // By doing it here, the async loop only does network I/O — no CPU stalls.
    // Channel carries (wire_frame, plaintext_len) so the async side can track progress.
    let (data_tx, mut data_rx) = tokio::sync::mpsc::channel::<(bytes::Bytes, usize)>(32);
    let path_buf_reader = path_buf.clone();
    let enc_key_reader = enc_key;
    let nonce_prefix_reader = nonce_prefix;

    let reader_task = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
        use std::io::{Read, Seek, SeekFrom};
        let mut file = std::fs::File::open(&path_buf_reader)?;
        let mut hasher = blake3::Hasher::new();

        let mut buf = vec![0u8; LAN_BLOCK_SIZE];
        let mut processed = 0u64;
        while processed < start_offset {
            let to_read = std::cmp::min(buf.len() as u64, start_offset - processed) as usize;
            let n = file.read(&mut buf[..to_read])?;
            if n == 0 { break; }
            hasher.update(&buf[..n]);
            processed += n as u64;
        }

        file.seek(SeekFrom::Start(start_offset))?;
        let mut reader = std::io::BufReader::with_capacity(4 * 1024 * 1024, file);
        let mut block_idx: u64 = start_offset / LAN_BLOCK_SIZE as u64;

        loop {
            let mut block = vec![0u8; LAN_BLOCK_SIZE];
            let n = reader.read(&mut block)?;
            if n == 0 { break; }
            block.truncate(n);
            hasher.update(&block);

            // Encrypt on this blocking thread — keeps the async task free for I/O
            let nonce = p2ptransfer_core::crypto::aead::build_nonce(&nonce_prefix_reader, block_idx);
            let encrypted = p2ptransfer_core::crypto::aead::encrypt(&enc_key_reader, &nonce, &block)
                .map_err(|e| anyhow::anyhow!("Encryption failed: {e}"))?;

            // Build the complete wire frame: tag(1) + block_index(4) + compressed_flag(1) + ciphertext
            let mut payload = Vec::with_capacity(6 + encrypted.len());
            payload.push(0x01u8); // TAG_CHUNK
            payload.extend_from_slice(&(block_idx as u32).to_le_bytes());
            payload.push(0u8); // not compressed
            payload.extend_from_slice(&encrypted);

            let plain_len = n;
            if data_tx.blocking_send((bytes::Bytes::from(payload), plain_len)).is_err() {
                break; // Network side dropped — abort
            }
            block_idx += 1;
        }
        Ok(hasher.finalize().to_hex().to_string())
    });

    let mut bytes_sent: u64 = start_offset;
    let mut window_bytes: u64 = 0;
    let mut window_start = std::time::Instant::now();
    let mut current_speed: f64 = 0.0;
    let mut last_progress = std::time::Instant::now();
    let total_size = metadata.file_size;
    // Async loop: just dequeue pre-encrypted frames and write to TCP — zero CPU work.
    while let Some((wire_frame, plain_len)) = data_rx.recv().await {
        p2ptransfer_core::network::tcp::send_message(&mut stream, &wire_frame)
            .await.map_err(|e| e.to_string())?;

        bytes_sent += plain_len as u64;
        window_bytes += plain_len as u64;

        let elapsed = window_start.elapsed();
        if elapsed.as_secs_f64() >= 0.5 {
            current_speed = window_bytes as f64 / elapsed.as_secs_f64();
            window_bytes = 0;
            window_start = std::time::Instant::now();
        }

        if last_progress.elapsed() > std::time::Duration::from_millis(50) {
            if let Some(mut state) = protocol::ACTIVE_TRANSFERS.get_mut(&request_id) {
                state.bytes_transferred = bytes_sent;
            }
            let _ = app_handle.emit_all(
                "send-progress",
                protocol::TransferProgressEvent {
                    request_id: request_id.clone(),
                    bytes_transferred: bytes_sent,
                    total: total_size,
                    speed_bytes_per_sec: current_speed,
                },
            );
            last_progress = std::time::Instant::now();
        }
    }

    let final_hash = reader_task.await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;

    // â”€â”€ Final handshake: send COMPLETE, wait for receiver's COMPLETE ACK â”€â”€
    let mut comp_msg = vec![TAG_COMPLETE];
    comp_msg.extend_from_slice(final_hash.as_bytes());
    p2ptransfer_core::network::tcp::send_message(&mut stream, &comp_msg)
        .await.map_err(|e| e.to_string())?;

    // Receiver verifies the checksum and responds with COMPLETE or ERROR.
    let comp_ack = p2ptransfer_core::network::tcp::receive_message(&mut stream)
        .await.map_err(|e| e.to_string())?;
    if comp_ack.is_empty() {
        return Err("No response from receiver".to_string());
    }
    if comp_ack[0] == TAG_ERROR {
        let msg = String::from_utf8_lossy(&comp_ack[1..]).to_string();
        return Err(format!("Receiver error: {msg}"));
    }
    if comp_ack[0] != TAG_COMPLETE {
        return Err("Expected COMPLETE ACK".to_string());
    }

    Ok(final_hash)
}
