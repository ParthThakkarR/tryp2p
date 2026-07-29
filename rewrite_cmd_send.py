import re

with open('desktop/p2ptransfer-cli/src/main.rs', 'r', encoding='utf-8') as f:
    content = f.read()

new_cmd_send = '''async fn cmd_send(
    path: PathBuf,
    peer: String,
    compression: i32,
    chunk_size: usize,
    relay: Option<String>,
    connections: Option<usize>,
    config: &P2pConfig,
) -> Result<()> {
    if !path.exists() {
        anyhow::bail!("Path does not exist: {}", path.display());
    }

    let relay_addr = relay
        .or_else(|| config.relay_server.clone())
        .and_then(|s| s.parse::<SocketAddr>().ok());

    let mut resolved_peer = peer.clone();
    let mut resolved_peer_id = None;
    
    if let Ok(resume_manager) = TransferResumeManager::new(config.data_dir.join("resume")) {
        if let Ok(Some(contact)) = resume_manager.get_contact(&peer) {
            let addr_str = format!("{}:{}", contact.last_known_ip, contact.last_known_port);
            println!("Resolved contact '{}' to {}", peer, addr_str);
            resolved_peer = addr_str;
            resolved_peer_id = Some(contact.peer_id);
        }
    }

    let peer_addr_opt: Option<SocketAddr> = resolved_peer.parse().ok();

    let mut stream = if let Some(peer_addr) = peer_addr_opt {
        tcp::connect(peer_addr).await?
    } else {
        anyhow::bail!("Cannot resolve peer");
    };
    
    // Explicit socket2 tuning
    let sock = socket2::SockRef::from(&stream);
    let _ = sock.set_send_buffer_size(4 * 1024 * 1024);
    let _ = sock.set_recv_buffer_size(4 * 1024 * 1024);
    set_nodelay(&stream);

    println!("Performing ECDH key exchange...");
    let start_rtt = std::time::Instant::now();
    let kx = EcdhKeyExchange::new();
    let client_pub = kx.public_key_bytes();
    send_tagged(&mut stream, TAG_CLIENT_HELLO, &client_pub).await?;
    let (hello_tag, server_pub_raw) = receive_tagged(&mut stream).await?;
    let rtt_ms = start_rtt.elapsed().as_secs_f64() * 1000.0;
    
    if hello_tag != TAG_SERVER_HELLO {
        anyhow::bail!("Expected SERVER_HELLO during handshake, got tag={hello_tag}");
    }
    let server_pub_bytes: [u8; 32] = server_pub_raw.as_slice().try_into().map_err(|_| anyhow::anyhow!("Invalid server public key length"))?;
    let shared_secret = kx.derive_shared_secret(&server_pub_bytes)?;
    let enc_key = aead::derive_encryption_key(&shared_secret, b"P2PTRANSFER_SALT_v1", b"p2ptransfer-v1-encryption")?;
    let nonce_prefix = aead::generate_nonce_prefix();
    info!("Key exchange complete (RTT: {:.2}ms)", rtt_ms);

    let engine = Arc::new(TransferEngine::new(4));
    let mut actual_chunk_size = chunk_size;
    if chunk_size == default_chunk_size() {
        actual_chunk_size = p2ptransfer_core::transfer::chunker::Chunker::adaptive_chunk_size(rtt_ms);
        println!("Adaptive chunk size set to {}", indicatif::HumanBytes(actual_chunk_size as u64));
    }
    
    let actual_compression = if compression == 10 {
        if rtt_ms < 5.0 { 0 } else { 6 }
    } else {
        compression
    };
    if actual_compression == 0 {
        println!("Compression disabled for fast LAN transfer");
    } else {
        println!("Using zstd compression level {}", actual_compression);
    }

    let mut metadata = engine.create_metadata(&path, actual_chunk_size).await?;
    metadata.nonce_prefix = nonce_prefix;

    println!(
        "Sending: {} ({} chunks, {} total)",
        metadata.file_name,
        metadata.total_chunks,
        HumanBytes(metadata.file_size)
    );

    let pb = ProgressBar::new(metadata.file_size);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
            .unwrap()
            .progress_chars("#>-"),
    );

    let resume_manager = Arc::new(TransferResumeManager::new(config.data_dir.join("resume"))?);
    let existing = resume_manager.list_transfers()?.into_iter().find(|t| {
        let p = std::path::Path::new(&t.file_path);
        p == path && t.peer_addr.contains(&peer) && !matches!(t.status, TransferStatus::Completed | TransferStatus::Failed)
    });

    let (session_id, resume_offset) = if let Some(record) = existing {
        let offset = record.bytes_transferred.max(0) as u64;
        let start_chunk = offset / metadata.chunk_size as u64;
        println!("Resuming transfer '{}' from chunk {}", record.id, start_chunk);
        (record.id, start_chunk)
    } else {
        let session_id = resume_manager.start_transfer(&path.to_string_lossy(), &peer, metadata.file_size as i64, true)?;
        (session_id, 0)
    };

    let meta_json = serde_json::to_vec(&metadata)?;
    send_tagged(&mut stream, TAG_METADATA, &meta_json).await?;
    let (tag, ack) = receive_tagged(&mut stream).await?;
    if tag == TAG_ERROR { anyhow::bail!("Peer rejected transfer: {}", String::from_utf8_lossy(&ack)); }
    if tag != TAG_ACCEPT { anyhow::bail!("Expected ACCEPT, got tag={tag}"); }

    let start_chunk = resume_offset;
    if resume_offset > 0 { pb.inc(resume_offset * metadata.chunk_size as u64); }

    let paused = Arc::new(AtomicBool::new(false));
    let paused_clone = paused.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        paused_clone.store(true, Ordering::SeqCst);
        eprintln!("\\n[Pause requested — finishing current chunk, then saving state...]");
    });

    let (net_tx, mut net_rx) = tokio::sync::mpsc::channel::<(u64, Vec<u8>)>(8);
    let (err_tx, mut err_rx) = tokio::sync::mpsc::channel::<String>(1);

    let path_clone = path.clone();
    let metadata_clone = metadata.clone();
    let engine_clone = engine.clone();
    let err_tx_clone = err_tx.clone();

    // Pipelined Send: Disk Reader & Encryptor task
    tokio::spawn(async move {
        for chunk_index in start_chunk..metadata_clone.total_chunks {
            if paused.load(Ordering::SeqCst) { break; }
            
            let chunk_data = match engine_clone.prepare_chunk(&path_clone, &metadata_clone, chunk_index).await {
                Ok(d) => d,
                Err(e) => { let _ = err_tx_clone.send(format!("Disk read failed: {e}")).await; break; }
            };
            
            let check_len = std::cmp::min(chunk_data.len(), 64);
            let should_compress = chunk_data.len() >= 64 && !detector::is_likely_compressed(&path_clone, &chunk_data[..check_len]);
            
            let (payload, flag) = if should_compress && actual_compression > 0 {
                match p2ptransfer_core::compress::zstd::compress(&chunk_data, actual_compression) {
                    Ok(c) => (c, 1u8),
                    Err(_) => (chunk_data, 0u8),
                }
            } else {
                (chunk_data, 0u8)
            };
            
            let nonce = aead::build_nonce(&nonce_prefix, chunk_index);
            let encrypted = match aead::encrypt(&enc_key, &nonce, &payload) {
                Ok(e) => e,
                Err(e) => { let _ = err_tx_clone.send(format!("Encryption failed: {e}")).await; break; }
            };
            
            let mut chunk_frame = Vec::with_capacity(5 + encrypted.len());
            chunk_frame.extend_from_slice(&(chunk_index as u32).to_le_bytes());
            chunk_frame.push(flag);
            chunk_frame.extend_from_slice(&encrypted);
            
            if net_tx.send((chunk_index, chunk_frame)).await.is_err() { break; }
        }
    });

    let mut bytes_sent = resume_offset * metadata.chunk_size as u64;
    
    // Network Sender loop
    loop {
        tokio::select! {
            Some((chunk_index, chunk_frame)) = net_rx.recv() => {
                if let Err(e) = send_tagged(&mut stream, TAG_CHUNK, &chunk_frame).await {
                    anyhow::bail!("Network send failed: {e}");
                }
                match receive_tagged(&mut stream).await {
                    Ok((TAG_CHUNK_ACK, _ack)) => {
                        let len = chunk_frame.len() as u64; // approximate original chunk size logic would use chunk_size
                        let actual_len = if chunk_index == metadata.total_chunks - 1 {
                            metadata.file_size % metadata.chunk_size as u64
                        } else {
                            metadata.chunk_size as u64
                        };
                        let len_to_inc = if actual_len == 0 { metadata.chunk_size as u64 } else { actual_len };
                        
                        pb.inc(len_to_inc);
                        bytes_sent += len_to_inc;
                        let _ = resume_manager.update_progress(&session_id, bytes_sent as i64);
                    }
                    Ok((TAG_ERROR, ack)) => anyhow::bail!("Peer error: {}", String::from_utf8_lossy(&ack)),
                    Ok((tag, _)) => anyhow::bail!("Expected CHUNK_ACK, got tag={tag}"),
                    Err(e) => anyhow::bail!("Failed to receive ACK: {e}"),
                }
            }
            Some(err) = err_rx.recv() => {
                anyhow::bail!("Pipeline error: {err}");
            }
            else => {
                break; // net_rx closed, meaning all chunks processed
            }
        }
    }

    if !paused_clone.load(Ordering::SeqCst) {
        pb.finish_with_message("Transfer complete");
        resume_manager.complete_transfer(&session_id, &format_hex(&metadata.checksum))?;
        println!("Transfer complete");
    }

    Ok(())
}'''

# Replace from `async fn cmd_send` to the end of the function.
# We will use regex to find the function.
pattern = re.compile(r'async fn cmd_send\(.*?\)\s*->\s*Result<\(\)>\s*\{(?:[^{}]*|\{(?:[^{}]*|\{[^{}]*\})*\})*\}', re.DOTALL)

match = pattern.search(content)
if match:
    # but the regex might not capture the whole function if the nesting is too deep.
    pass

# safer way: find async fn cmd_send, then count braces
lines = content.splitlines(True)
start_idx = -1
for i, line in enumerate(lines):
    if line.startswith('async fn cmd_send('):
        start_idx = i
        break

if start_idx != -1:
    brace_count = 0
    end_idx = -1
    for i in range(start_idx, len(lines)):
        brace_count += lines[i].count('{')
        brace_count -= lines[i].count('}')
        if brace_count == 0 and lines[i].count('{') > 0 or (brace_count == 0 and i > start_idx + 5):
            end_idx = i
            break
            
    if end_idx != -1:
        new_content = "".join(lines[:start_idx]) + new_cmd_send + "\n" + "".join(lines[end_idx+1:])
        with open('desktop/p2ptransfer-cli/src/main.rs', 'w', encoding='utf-8') as f:
            f.write(new_content)
        print("Replaced cmd_send")
    else:
        print("Could not find end of cmd_send")
else:
    print("Could not find start of cmd_send")
