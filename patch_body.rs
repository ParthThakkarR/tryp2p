    // --- Signal handling for graceful pause ---
    let paused = Arc::new(AtomicBool::new(false));
    let paused_clone = paused.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        paused_clone.store(true, Ordering::SeqCst);
        eprintln!("\n[Pause requested — finishing current chunk, then saving state...]");
    });

    let mut current_start_chunk = start_chunk;
    let bytes_sent_atomic = Arc::new(std::sync::atomic::AtomicI64::new(resume_offset as i64));

    loop {
        if paused.load(Ordering::SeqCst) {
            println!(
                "\nTransfer paused at chunk {}/{} ({} bytes). Resume with same command.",
                current_start_chunk,
                metadata.total_chunks,
                HumanBytes(bytes_sent_atomic.load(Ordering::SeqCst) as u64)
            );
            return Ok(());
        }

        let window_size = 16;
        let global_semaphore = Arc::new(tokio::sync::Semaphore::new(window_size));
        let (ack_tx, mut ack_rx) = tokio::sync::mpsc::channel(window_size * 2);
        let (error_tx, mut error_rx) = tokio::sync::mpsc::channel(connections);
        
        let mut streams = vec![stream];
        for _ in 1..connections {
            if let Ok(mut aux_stream) = tcp::connect(peer_addr).await {
                set_nodelay(&aux_stream);
                let kx = EcdhKeyExchange::new();
                let client_pub = kx.public_key_bytes();
                if send_tagged(&mut aux_stream, TAG_CLIENT_HELLO, &client_pub).await.is_ok() {
                    if let Ok((hello_tag, server_pub_raw)) = receive_tagged(&mut aux_stream).await {
                        if hello_tag == TAG_SERVER_HELLO {
                            if send_tagged(&mut aux_stream, TAG_SESSION_JOIN, session_id.as_bytes()).await.is_ok() {
                                if let Ok((tag, response)) = receive_tagged(&mut aux_stream).await {
                                    if tag == TAG_METADATA && response == b"ACCEPT" {
                                        streams.push(aux_stream);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        let next_chunk = Arc::new(std::sync::atomic::AtomicU64::new(current_start_chunk));
        let mut join_set = tokio::task::JoinSet::new();
        
        for mut stream in streams.into_iter() {
            let (mut rh, mut wh) = stream.into_split();
            let ack_tx_clone = ack_tx.clone();
            let error_tx_clone = error_tx.clone();
            
            join_set.spawn(async move {
                loop {
                    match receive_tagged(&mut rh).await {
                        Ok((tag, payload)) => {
                            if tag == TAG_CHUNK_ACK {
                                if payload.len() >= 4 {
                                    let acked_index = u32::from_le_bytes(payload[..4].try_into().unwrap()) as u64;
                                    if ack_tx_clone.send(acked_index).await.is_err() { break; }
                                }
                            } else if tag == TAG_COMPLETE || tag == TAG_ERROR {
                                let _ = error_tx_clone.send((tag, payload)).await;
                                break;
                            }
                        }
                        Err(e) => {
                            let _ = error_tx_clone.send((TAG_ERROR, e.to_string().into_bytes())).await;
                            break;
                        }
                    }
                }
            });
            
            let semaphore = global_semaphore.clone();
            let next_chunk = next_chunk.clone();
            let engine = engine.clone();
            let path = file_path.clone();
            let metadata = metadata.clone();
            let enc_key = enc_key;
            let nonce_prefix = nonce_prefix;
            let error_tx_clone2 = error_tx.clone();
            
            join_set.spawn(async move {
                loop {
                    let permit = match semaphore.clone().acquire_owned().await {
                        Ok(p) => p,
                        Err(_) => break,
                    };
                    
                    let chunk_index = next_chunk.fetch_add(1, Ordering::SeqCst);
                    if chunk_index >= metadata.total_chunks {
                        break;
                    }
                    
                    let chunk_data_res = engine.prepare_chunk(&path, &metadata, chunk_index).await;
                    if let Ok(chunk_data) = chunk_data_res {
                        let chunk_len = chunk_data.len();
                        let should_compress = chunk_len >= 64 && !p2ptransfer_core::compress::detector::is_likely_compressed(&path, &chunk_data);
                        
                        let frame_res = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
                            let payload = if should_compress && compression > 0 {
                                p2ptransfer_core::compress::zstd::compress(&chunk_data, compression)?
                            } else {
                                chunk_data
                            };
                            let compressed_flag: u8 = if should_compress && compression > 0 { 1 } else { 0 };
                            let nonce = aead::build_nonce(&nonce_prefix, chunk_index);
                            let encrypted_payload = aead::encrypt(&enc_key, &nonce, &payload)?;
                            
                            let mut chunk_frame = Vec::with_capacity(5 + encrypted_payload.len());
                            chunk_frame.extend_from_slice(&(chunk_index as u32).to_le_bytes());
                            chunk_frame.push(compressed_flag);
                            chunk_frame.extend_from_slice(&encrypted_payload);
                            
                            Ok(chunk_frame)
                        }).await;
                        
                        match frame_res {
                            Ok(Ok(frame)) => {
                                if send_tagged(&mut wh, TAG_CHUNK, &frame).await.is_err() {
                                    let _ = error_tx_clone2.send((TAG_ERROR, b"Write failed".to_vec())).await;
                                    break;
                                }
                                permit.forget();
                            }
                            _ => {
                                let _ = error_tx_clone2.send((TAG_ERROR, b"Prepare failed".to_vec())).await;
                                break;
                            }
                        }
                    } else {
                        let _ = error_tx_clone2.send((TAG_ERROR, b"Read failed".to_vec())).await;
                        break;
                    }
                }
            });
        }
        
        let mut acks_received = 0;
        let total_to_send = metadata.total_chunks - current_start_chunk;
        let mut pipeline_error = None;
        let mut final_complete = None;
        
        loop {
            if acks_received == total_to_send {
                if final_complete.is_some() { break; }
            }
            
            tokio::select! {
                Some(acked_idx) = ack_rx.recv() => {
                    acks_received += 1;
                    current_start_chunk = current_start_chunk.max(acked_idx + 1);
                    global_semaphore.add_permits(1);
                    
                    pb.inc(metadata.chunk_size as u64);
                    bytes_sent_atomic.fetch_add(metadata.chunk_size as i64, Ordering::SeqCst);
                    let _ = resume_manager.update_progress(&session_id, bytes_sent_atomic.load(Ordering::SeqCst));
                    
                    if acks_received == total_to_send && final_complete.is_some() {
                        break;
                    }
                }
                Some((tag, payload)) = error_rx.recv() => {
                    if tag == TAG_COMPLETE {
                        final_complete = Some(payload);
                        if acks_received == total_to_send { break; }
                    } else {
                        pipeline_error = Some(anyhow::anyhow!("Pipeline error: {}", String::from_utf8_lossy(&payload)));
                        break;
                    }
                }
            }
        }
        
        join_set.abort_all();
        
        if let Some(e) = pipeline_error {
            warn!("Pipeline broken: {e:?}. Reconnecting in 2s...");
            tokio::time::sleep(Duration::from_secs(2)).await;
            return Err(anyhow::anyhow!("Transfer interrupted, please restart to resume"));
        }
        
        if let Some(response) = final_complete {
            let recv_hash = &response[..response.len().min(32)];
            let expected = &metadata.checksum[..];
            if recv_hash != expected {
                resume_manager.fail_transfer(&session_id)?;
                anyhow::bail!("Checksum mismatch: receiver hash differs from sender");
            }
            pb.finish_with_message("Transfer verified");
            resume_manager.complete_transfer(&session_id, &format_hex(&metadata.checksum))?;
            println!("Transfer complete and verified by receiver");
        } else {
            resume_manager.fail_transfer(&session_id)?;
            anyhow::bail!("Transfer ended without completion tag");
        }
        break;
    }
    
    Ok(())
}