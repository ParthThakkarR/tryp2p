with open('desktop/p2ptransfer-cli/src/main.rs', 'r', encoding='utf-8') as f:
    lines = f.readlines()
insertion = '''    let peer_addr_opt: Option<SocketAddr> = resolved_peer.parse().ok();

    let mut stream = if let Some(peer_addr) = peer_addr_opt {
        match try_connect_fallback(peer_addr, config.socket_buffer_size).await {
            Ok(s) => s,
            Err(e) => anyhow::bail!("{e}"),
        }
    } else {
        anyhow::bail!("Cannot resolve peer");
    };
    set_nodelay(&stream);

    // --- ECDH Handshake ---
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
    let server_pub_bytes: [u8; 32] = server_pub_raw
        .as_slice()
        .try_into()
        .map_err(|_| anyhow::anyhow!("Invalid server public key length"))?;
    let shared_secret = kx.derive_shared_secret(&server_pub_bytes)?;
    let mut enc_key =
        aead::derive_encryption_key(&shared_secret, b"P2PTRANSFER_SALT_v1", b"p2ptransfer-v1-encryption")?;
    let mut nonce_prefix = aead::generate_nonce_prefix();
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
        indicatif::HumanBytes(metadata.file_size)
'''
index = -1
for i, line in enumerate(lines):
    if i > 485 and "    );" in line:
        index = i
        break

if index != -1:
    lines[index] = insertion + '\n    );\n'
    if lines[index-1].strip() == "":
        lines.pop(index-1)
    with open('desktop/p2ptransfer-cli/src/main.rs', 'w', encoding='utf-8') as f:
        f.writelines(lines)
else:
    print("Could not find );")
