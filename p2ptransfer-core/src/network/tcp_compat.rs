use anyhow::Result;
use std::net::SocketAddr;
use crate::network::alpn::{TransferEvent, SendStatusEvent, TransferProgressEvent};
use crate::transfer::engine::TransferEngine;

#[allow(dead_code)]
const TAG_CHUNK: u8 = 0x01;
#[allow(dead_code)]
const TAG_CHUNK_ACK: u8 = 0x02;
const TAG_COMPLETE: u8 = 0x03;
const TAG_ERROR: u8 = 0x04;
const TAG_CLIENT_HELLO: u8 = 0x05;
const TAG_SERVER_HELLO: u8 = 0x06;
const TAG_METADATA: u8 = 0x00;

const LAN_BLOCK_SIZE: usize = 4 * 1024 * 1024;
pub const CHANNEL_CAPACITY: usize = 32;

pub async fn handle_incoming(
    mut stream: tokio::net::TcpStream,
    addr: SocketAddr,
    output_dir: std::path::PathBuf,
    event_tx: &tokio::sync::mpsc::Sender<TransferEvent>,
    pending: crate::network::alpn::PendingRequests,
) -> Result<(), String> {
    let _ = crate::network::tcp::configure_socket(&stream);
    let send_err = |stream: &mut tokio::net::TcpStream, msg: &str| {
        let _ = crate::network::tcp::send_message(stream, format!("ERROR:{msg}").as_bytes());
    };

    let data = crate::network::tcp::receive_message(&mut stream).await.map_err(|e| {
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
    let kx = crate::crypto::ecdh::EcdhKeyExchange::new();
    let server_pub = kx.public_key_bytes();
    let mut hello_frame = Vec::with_capacity(1 + server_pub.len());
    hello_frame.push(0x06);
    hello_frame.extend_from_slice(&server_pub);
    crate::network::tcp::send_message(&mut stream, &hello_frame).await.map_err(|e| {
        let msg = format!("Failed to send SERVER_HELLO to {addr}: {e}");
        eprintln!("{msg}");
        msg
    })?;
    let shared_secret = kx.derive_shared_secret(&client_pub_bytes).map_err(|e| e.to_string())?;
    let enc_key = crate::crypto::aead::derive_encryption_key(&shared_secret, b"P2PTRANSFER_SALT_v1", b"p2ptransfer-v1-encryption")
        .map_err(|e| e.to_string())?;

    let meta_data = crate::network::tcp::receive_message(&mut stream).await.map_err(|e| {
        let msg = format!("Failed to read METADATA from {addr}: {e}");
        eprintln!("{msg}");
        msg
    })?;
    if meta_data.is_empty() || meta_data[0] != 0x00 {
        send_err(&mut stream, "Expected METADATA");
        return Err("Expected METADATA".into());
    }
    let metadata: crate::transfer::engine::TransferMetadata =
        serde_json::from_slice(&meta_data[1..]).map_err(|e| {
            send_err(&mut stream, "Invalid metadata");
            e.to_string()
        })?;
    let nonce_prefix = metadata.nonce_prefix;
    eprintln!("Incoming transfer: {} from {addr}", metadata.file_name);

    let request_id = uuid::Uuid::new_v4().to_string();
    let rx = pending.register(request_id.clone()).await;

    let _ = event_tx.send(TransferEvent::Incoming(crate::network::alpn::IncomingTransferEvent { request_id:  request_id.clone(), sender_name:  addr.to_string(), sender_node_id:  String::new(), file_name:  metadata.file_name.clone(), file_size:  metadata.file_size })).await;

    

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

    let (resume_offset, is_resume) = if !metadata.force_restart && existing_len > 0 && existing_len < metadata.file_size {
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
    crate::network::tcp::send_message(&mut stream, &accept_msg).await.map_err(|e| {
        let msg = format!("Failed to send ACCEPT to {addr}: {e}");
        eprintln!("{msg}");
        msg
    })?;

    // ── Dedicated spawn_blocking Decrypt + Disk Writer Task ──────────────
    // Decryption (ChaCha20-Poly1305 on 4 MB blocks) is CPU-bound.
    // By combining decrypt + disk write in one blocking task, the async loop
    // is free to read the next network message while decryption runs.
    // Channel carries: (block_index, encrypted_payload_from_byte_6_onward)
    let (disk_tx, mut disk_rx) = tokio::sync::mpsc::channel::<(u64, Vec<u8>)>(CHANNEL_CAPACITY);
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
            let nonce = crate::crypto::aead::build_nonce(&nonce_prefix_writer, blk_idx);
            let decrypted = crate::crypto::aead::decrypt(&enc_key_writer, &nonce, &encrypted_data)
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

    crate::network::alpn::ACTIVE_TRANSFERS.insert(request_id.clone(), crate::network::alpn::ActiveTransferState {
        request_id: request_id.clone(),
        role: "receiver".to_string(),
        status: "transferring".to_string(),
        file_name: metadata.file_name.clone(),
        total_bytes: metadata.file_size,
        bytes_transferred: resume_offset,
    });

    // Register a pause/cancel flag so the desktop cancel_transfer command can interrupt us
    let pause_flag = crate::network::alpn::PauseFlag::new();
    crate::network::alpn::TRANSFER_PAUSE_FLAGS.insert(request_id.clone(), pause_flag.clone());

    // Async loop: reads framed messages and dispatches encrypted payloads.
    // Decryption happens in the blocking writer task above — zero CPU stalls here.
    let mut was_cancelled = false;
    let recv_loop_result: Result<(), String> = async {
        loop {
        // ── Check pause/cancel before reading next chunk ──
        if pause_flag.wait_if_paused().await {
            return Err("cancelled".to_string());
        }

        let msg_data = crate::network::tcp::receive_message(&mut stream).await.map_err(|e| {
            crate::network::alpn::ACTIVE_TRANSFERS.remove(&request_id);
            format!("Network read error from {addr}: {e}")
        })?;

        if msg_data.is_empty() {
            crate::network::alpn::ACTIVE_TRANSFERS.remove(&request_id);
            send_err(&mut stream, "Empty message");
            return Err("Empty message received".into());
        }

        match msg_data[0] {
            0x01 => {
                if msg_data.len() < 6 {
                    crate::network::alpn::ACTIVE_TRANSFERS.remove(&request_id);
                    send_err(&mut stream, "Malformed chunk");
                    return Err("Malformed chunk frame".into());
                }

                // Estimate plaintext size from ciphertext (subtract 16-byte Poly1305 tag)
                let ciphertext_len = msg_data.len() - 6;
                let plaintext_len = ciphertext_len.saturating_sub(16);

                // Forward encrypted payload to the decrypt+write task
                if disk_tx.send((block_index, msg_data[6..].to_vec())).await.is_err() {
                    crate::network::alpn::ACTIVE_TRANSFERS.remove(&request_id);
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
                    if let Some(mut state) = crate::network::alpn::ACTIVE_TRANSFERS.get_mut(&request_id) {
                        state.bytes_transferred = bytes_received;
                    }
                    let _ = event_tx.send(TransferEvent::Progress(TransferProgressEvent { request_id:  request_id.clone(), bytes_transferred:  bytes_received, total:  metadata.file_size, speed_bytes_per_sec:  current_speed })).await;
                    last_progress = std::time::Instant::now();
                }

                block_index += 1;
            }
            0x03 => {
                received_hash = String::from_utf8_lossy(&msg_data[1..]).to_string();
                break;
            }
            0x04 => {
                crate::network::alpn::ACTIVE_TRANSFERS.remove(&request_id);
                let err = String::from_utf8_lossy(&msg_data[1..]).to_string();
                if err.contains("cancelled") || err.contains("Cancelled") {
                    return Err("cancelled".to_string());
                }
                return Err(format!("Sender error: {err}"));
            }
            tag => {
                crate::network::alpn::ACTIVE_TRANSFERS.remove(&request_id);
                crate::network::alpn::TRANSFER_PAUSE_FLAGS.remove(&request_id);
                send_err(&mut stream, &format!("Unexpected tag: {tag:#04x}"));
                return Err(format!("Unexpected message tag: {tag:#04x}"));
            }
        }
    }
    Ok(())
    }.await;

    // Drop disk_tx to signal completion to writer task (whether success, error, or cancel)
    drop(disk_tx);
    // Clean up pause flag
    crate::network::alpn::TRANSFER_PAUSE_FLAGS.remove(&request_id);

    if let Err(ref e) = recv_loop_result {
        if e == "cancelled" || e.contains("cancelled") || e.contains("Cancelled") {
            was_cancelled = true;
            crate::network::alpn::ACTIVE_TRANSFERS.remove(&request_id);
            let _ = event_tx.send(TransferEvent::Cancelled { request_id: request_id.clone() }).await;
        } else {
            // Wait for writer task to drain before returning error and cleanup partial file
            let _ = writer_task.await;
            let _ = std::fs::remove_file(&output_path);
            return Err(recv_loop_result.unwrap_err());
        }
    }

    if was_cancelled {
        let mut err_frame = vec![0x04u8];
        err_frame.extend_from_slice(b"Transfer cancelled by user");
        let _ = crate::network::tcp::send_message(&mut stream, &err_frame).await;
        let _ = writer_task.await;
        let _ = std::fs::remove_file(&output_path);
        return Err("Transfer cancelled by user".to_string());
    }

    // Wait for writer task to finish flushing to disk
    match writer_task.await {
        Ok(Ok(_)) => {},
        _ => {
            crate::network::alpn::ACTIVE_TRANSFERS.remove(&request_id);
            let _ = std::fs::remove_file(&output_path);
            return Err("Disk flush failed".into());
        }
    }

    let engine = TransferEngine::new(4);
    let valid = engine
        .verify_checksum(&output_path, &metadata.checksum)
        .await
        .map_err(|e| {
            crate::network::alpn::ACTIVE_TRANSFERS.remove(&request_id);
            let _ = std::fs::remove_file(&output_path);
            e.to_string()
        })?;

    if valid {
        let mut complete = vec![0x03u8];
        complete.extend_from_slice(received_hash.as_bytes());
        crate::network::tcp::send_message(&mut stream, &complete).await.map_err(|e| {
            let msg = format!("Failed to send COMPLETE ACK to {addr}: {e}");
            eprintln!("{msg}");
            msg
        })?;
        
        let elapsed = start_time.elapsed().as_secs_f64();
        let _ = event_tx.send(TransferEvent::Complete(crate::network::alpn::TransferCompleteEvent { request_id: request_id.clone(), 
                file_path: output_path.to_string_lossy().to_string(),
                blake3_hash: blake3::Hash::from(metadata.checksum).to_hex().to_string(),
                elapsed_secs: elapsed,
             })).await;
        crate::network::alpn::ACTIVE_TRANSFERS.remove(&request_id);
        eprintln!("Transfer complete from {addr}: {} ({block_index} blocks)", metadata.file_name);
    } else {
        crate::network::alpn::ACTIVE_TRANSFERS.remove(&request_id);
        let _ = std::fs::remove_file(&output_path);
        let err_msg = format!("Checksum mismatch for {} from {addr}", metadata.file_name);
        eprintln!("{err_msg}");
        let mut error_resp = vec![0x04u8];
        error_resp.extend_from_slice(b"Checksum mismatch");
        let _ = crate::network::tcp::send_message(&mut stream, &error_resp).await;
        return Err(err_msg);
    }
    
    Ok(())
}

pub async fn send_file_direct_tcp(
    request_id: String,
    path: String,
    target_ip: &str,
    target_port: u16,
    event_tx: &tokio::sync::mpsc::Sender<TransferEvent>,
    force_restart: bool,
) -> Result<String, String> {
    let _ = event_tx.send(TransferEvent::SendStatus(SendStatusEvent { request_id:  request_id.clone(), status:  "connecting".to_string() })).await;

    let addr_str = format!("{}:{}", target_ip, target_port);
    let peer_addr = addr_str.parse::<std::net::SocketAddr>().map_err(|e| e.to_string())?;

    // Connect with 32 MB socket buffers for multi-gigabit throughput.
    let direct_result = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        crate::network::tcp::connect(peer_addr, 32 * 1024 * 1024),
    ).await;

    let mut stream = match direct_result {
        Ok(Ok(s)) => s,
        _ => {
            let msg = format!(
                "Failed to connect to {peer_addr}.\n\nCheck that the receiver is listening on port {} at that IP address.",
                peer_addr.port()
            );
            crate::network::alpn::emit_transfer_error(event_tx, &request_id, &msg);
            return Err(msg);
        }
    };

    // Register pause/cancel flag so UI can pause/cancel TCP sends too
    let pause_flag = crate::network::alpn::PauseFlag::new();
    crate::network::alpn::TRANSFER_PAUSE_FLAGS.insert(request_id.clone(), pause_flag.clone());
    crate::network::alpn::ACTIVE_TRANSFERS.insert(request_id.clone(), crate::network::alpn::ActiveTransferState {
        request_id: request_id.clone(),
        role: "sender".to_string(),
        status: "connecting".to_string(),
        file_name: std::path::Path::new(&path).file_name().unwrap_or_default().to_string_lossy().to_string(),
        total_bytes: std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0),
        bytes_transferred: 0,
    });

    let _ = event_tx.send(TransferEvent::SendStatus(SendStatusEvent { request_id:  request_id.clone(), status:  "accepted".to_string() })).await;

    // ── ECDH key exchange ────────────────────────────────────────────────────────
    let kx = crate::crypto::ecdh::EcdhKeyExchange::new();
    let client_pub = kx.public_key_bytes();
    let mut hello_msg = vec![TAG_CLIENT_HELLO];
    hello_msg.extend_from_slice(&client_pub);
    crate::network::tcp::send_message(&mut stream, &hello_msg)
        .await.map_err(|e| e.to_string())?;

    let resp = crate::network::tcp::receive_message(&mut stream)
        .await.map_err(|e| e.to_string())?;
    if resp.is_empty() || resp[0] != TAG_SERVER_HELLO {
        return Err("Expected SERVER_HELLO".to_string());
    }
    let server_pub_bytes: [u8; 32] = resp[1..]
        .try_into().map_err(|_| "Invalid key length".to_string())?;
    let shared_secret = kx
        .derive_shared_secret(&server_pub_bytes).map_err(|e| e.to_string())?;
    let enc_key = crate::crypto::aead::derive_encryption_key(
        &shared_secret, b"P2PTRANSFER_SALT_v1", b"p2ptransfer-v1-encryption"
    ).map_err(|e| e.to_string())?;
    let nonce_prefix = crate::crypto::aead::generate_nonce_prefix();

    // ── File metadata ────────────────────────────────────────────────────────────
    let path_buf = std::path::PathBuf::from(&path);
    let engine = std::sync::Arc::new(TransferEngine::new(4));
    let mut metadata = engine
        .create_metadata(&path_buf, LAN_BLOCK_SIZE, force_restart)
        .await.map_err(|e| e.to_string())?;
    metadata.nonce_prefix = nonce_prefix;

    let meta_json = serde_json::to_vec(&metadata).map_err(|e| e.to_string())?;
    let mut meta_msg = vec![TAG_METADATA];
    meta_msg.extend_from_slice(&meta_json);
    crate::network::tcp::send_message(&mut stream, &meta_msg)
        .await.map_err(|e| e.to_string())?;

    let resp2 = crate::network::tcp::receive_message(&mut stream)
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

    let _ = event_tx.send(TransferEvent::SendStatus(SendStatusEvent { request_id:  request_id.clone(), status:  "transferring".to_string() })).await;

    // ── Dedicated spawn_blocking File Reader + Encrypt Task ─────────────
    // Encryption (ChaCha20-Poly1305 on 4 MB blocks) is CPU-bound work.
    // By doing it here, the async loop only does network I/O — no CPU stalls.
    // Channel carries (wire_frame, plaintext_len) so the async side can track progress.
    let (data_tx, mut data_rx) = tokio::sync::mpsc::channel::<(bytes::Bytes, usize)>(CHANNEL_CAPACITY);
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
            let nonce = crate::crypto::aead::build_nonce(&nonce_prefix_reader, block_idx);
            let encrypted = crate::crypto::aead::encrypt(&enc_key_reader, &nonce, &block)
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

    // Async loop: dequeue pre-encrypted frames, check pause/cancel, write to TCP.
    let send_result: Result<(), String> = async {
        loop {
            // ── Step 1: Check pause/cancel BEFORE dequeuing next chunk ─────
            if pause_flag.wait_if_paused().await {
                return Err("Transfer cancelled by user".to_string());
            }

            // ── Step 2: Dequeue next chunk — cancel interruptible ──────────
            let next = tokio::select! {
                biased;
                _ = pause_flag.wait_for_cancel() => return Err("Transfer cancelled by user".to_string()),
                item = data_rx.recv() => item,
            };

            let (wire_frame, plain_len) = match next {
                Some(x) => x,
                None => break, // reader finished
            };

            crate::network::tcp::send_message(&mut stream, &wire_frame)
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
                if let Some(mut state) = crate::network::alpn::ACTIVE_TRANSFERS.get_mut(&request_id) {
                    state.bytes_transferred = bytes_sent;
                }
                let _ = event_tx.send(TransferEvent::Progress(TransferProgressEvent {
                    request_id: request_id.clone(),
                    bytes_transferred: bytes_sent,
                    total: total_size,
                    speed_bytes_per_sec: current_speed,
                })).await;
                last_progress = std::time::Instant::now();
            }
        }
        Ok(())
    }.await;

    // Clean up flags
    crate::network::alpn::TRANSFER_PAUSE_FLAGS.remove(&request_id);
    crate::network::alpn::ACTIVE_TRANSFERS.remove(&request_id);

    if let Err(cancel_msg) = send_result {
        let mut err_frame = vec![TAG_ERROR];
        err_frame.extend_from_slice(cancel_msg.as_bytes());
        let _ = crate::network::tcp::send_message(&mut stream, &err_frame).await;
        let _ = event_tx.send(TransferEvent::Cancelled { request_id: request_id.clone() }).await;
        return Err(cancel_msg);
    }

    let final_hash = reader_task.await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;

    // â”€â”€ Final handshake: send COMPLETE, wait for receiver's COMPLETE ACK â”€â”€
    let mut comp_msg = vec![TAG_COMPLETE];
    comp_msg.extend_from_slice(final_hash.as_bytes());
    crate::network::tcp::send_message(&mut stream, &comp_msg)
        .await.map_err(|e| e.to_string())?;

    // Receiver verifies the checksum and responds with COMPLETE or ERROR.
    let comp_ack = crate::network::tcp::receive_message(&mut stream)
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