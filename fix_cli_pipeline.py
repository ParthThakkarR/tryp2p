import sys

path = r"d:\tryp2p\desktop\p2ptransfer-cli\src\main.rs"
with open(path, "r", encoding="utf-8") as f:
    content = f.read()

# Replace the sequential send loop
start_marker = "let mut buffered_frames = std::collections::HashMap::new();"
end_marker = "let (tag, response) = receive_tagged(&mut stream).await?;"

start_idx = content.find(start_marker)
end_idx = content.find(end_marker)

if start_idx == -1 or end_idx == -1:
    print("Markers not found")
    sys.exit(1)

new_code = """let (mut read_half, mut write_half) = stream.into_split();

    let (ack_tx, mut ack_rx) = tokio::sync::mpsc::channel::<(u8, Vec<u8>)>(1024);
    tokio::spawn(async move {
        loop {
            match p2ptransfer_core::network::tcp::receive_tagged(&mut read_half).await {
                Ok(res) => {
                    if ack_tx.send(res).await.is_err() { break; }
                }
                Err(e) => {
                    let _ = ack_tx.send((p2ptransfer_core::network::tcp::TAG_ERROR, format!("Read failed: {e}").into_bytes())).await;
                    break;
                }
            }
        }
    });

    let mut buffered_frames = std::collections::HashMap::new();
    let mut next_to_send = start_chunk;
    let mut next_to_ack = start_chunk;
    let window_size = 32;

    loop {
        let in_flight = next_to_send - next_to_ack;
        
        tokio::select! {
            Some((chunk_index, chunk_frame)) = net_rx.recv(), if in_flight < window_size => {
                buffered_frames.insert(chunk_index, chunk_frame);
                
                while let Some(frame) = buffered_frames.remove(&next_to_send) {
                    if let Err(e) = p2ptransfer_core::network::tcp::send_tagged(&mut write_half, p2ptransfer_core::network::tcp::TAG_CHUNK, &frame).await {
                        resume_manager.fail_transfer(&session_id).ok();
                        anyhow::bail!("Network send failed: {e}");
                    }
                    next_to_send += 1;
                }
            }
            Some((tag, ack)) = ack_rx.recv() => {
                if tag == p2ptransfer_core::network::tcp::TAG_CHUNK_ACK {
                    let actual_len = if next_to_ack == metadata.total_chunks - 1 {
                        metadata.file_size % metadata.chunk_size as u64
                    } else {
                        metadata.chunk_size as u64
                    };
                    let len_to_inc = if actual_len == 0 { metadata.chunk_size as u64 } else { actual_len };
                    pb.inc(len_to_inc);
                    bytes_sent += len_to_inc as i64;
                    let _ = resume_manager.update_progress(&session_id, bytes_sent);
                    next_to_ack += 1;
                    
                    if next_to_ack == metadata.total_chunks {
                        break;
                    }
                } else if tag == p2ptransfer_core::network::tcp::TAG_ERROR {
                    resume_manager.fail_transfer(&session_id).ok();
                    anyhow::bail!("Peer error: {}", String::from_utf8_lossy(&ack))
                } else if tag == p2ptransfer_core::network::tcp::TAG_COMPLETE {
                    resume_manager.fail_transfer(&session_id).ok();
                    anyhow::bail!("Unexpected COMPLETE tag before all chunks acked")
                } else {
                    resume_manager.fail_transfer(&session_id).ok();
                    anyhow::bail!("Expected CHUNK_ACK, got tag={tag}")
                }
            }
            Some(err) = err_rx.recv() => {
                resume_manager.fail_transfer(&session_id).ok();
                anyhow::bail!("Pipeline error: {err}");
            }
            else => {
                break;
            }
        }
        
        if paused.load(Ordering::SeqCst) {
            break;
        }
    }
    
    if paused.load(Ordering::SeqCst) || next_to_ack < metadata.total_chunks {
        println!("\\nTransfer paused at chunk {}/{} ({} bytes). Resume with same command.", next_to_ack, metadata.total_chunks, HumanBytes(bytes_sent as u64));
        return Ok(());
    }

    // --- Wait for receiver verification ---
    let (tag, response) = match ack_rx.recv().await {
        Some(res) => res,
        None => anyhow::bail!("Connection closed before verification"),
    };
"""

content = content[:start_idx] + new_code + content[end_idx + len(end_marker):]

with open(path, "w", encoding="utf-8") as f:
    f.write(content)
