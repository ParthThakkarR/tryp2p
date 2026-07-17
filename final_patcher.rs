use std::fs;

fn main() {
    let mut s = fs::read_to_string("desktop/p2ptransfer-cli/src/main.rs").unwrap();

    // 1. Remove imports
    s = s.replace("use p2ptransfer_core::network::{nat, relay, stun};", "");

    // 2. Add connections to Send args
    let arg_chunk = "        /// Chunk size in bytes (default 16MB)\r\n        #[arg(long, default_value_t = 16 * 1024 * 1024)]\r\n        chunk_size: usize,\r\n        /// Relay server address (host:port) for cross-network transfers\r\n        #[arg(long)]\r\n        relay: Option<String>,";
    let arg_chunk_lf = arg_chunk.replace("\r\n", "\n");
    let rep_chunk = "        /// Chunk size in bytes (default 16MB)\n        #[arg(long, default_value_t = 16 * 1024 * 1024)]\n        chunk_size: usize,\n        /// Number of parallel connections\n        #[arg(long)]\n        connections: Option<usize>,\n        /// Relay server address (host:port) for cross-network transfers\n        #[arg(long)]\n        relay: Option<String>,";
    
    if s.contains(arg_chunk) { s = s.replace(arg_chunk, rep_chunk); }
    else if s.contains(&arg_chunk_lf) { s = s.replace(&arg_chunk_lf, rep_chunk); }

    let send_cmd = "        Commands::Send {\r\n            path,\r\n            peer,\r\n            compression,\r\n            chunk_size,\r\n            relay,\r\n        } => cmd_send(path, peer, compression, chunk_size, relay, &config).await,";
    let send_cmd_lf = send_cmd.replace("\r\n", "\n");
    let rep_send_cmd = "        Commands::Send {\n            path,\n            peer,\n            compression,\n            chunk_size,\n            relay,\n            connections,\n        } => cmd_send(path, peer, compression, chunk_size, relay, connections, &config).await,";

    if s.contains(send_cmd) { s = s.replace(send_cmd, rep_send_cmd); }
    else if s.contains(&send_cmd_lf) { s = s.replace(&send_cmd_lf, rep_send_cmd); }

    // 3. Fix P2pConfig
    s = s.replace(
        "fn default_data_dir() -> PathBuf {",
        "fn default_connections() -> usize { 4 }\n\nfn default_data_dir() -> PathBuf {"
    );
    let p2p_cfg = "    #[serde(default)]\r\n    relay_server: Option<String>,\r\n}";
    let p2p_cfg_lf = p2p_cfg.replace("\r\n", "\n");
    let rep_p2p_cfg = "    #[serde(default)]\n    relay_server: Option<String>,\n    #[serde(default = \"default_connections\")]\n    connections: usize,\n}";
    if s.contains(p2p_cfg) { s = s.replace(p2p_cfg, rep_p2p_cfg); }
    else if s.contains(&p2p_cfg_lf) { s = s.replace(&p2p_cfg_lf, rep_p2p_cfg); }

    let def_cfg = "            relay_server: None,\r\n        }\r\n    }\r\n}";
    let def_cfg_lf = def_cfg.replace("\r\n", "\n");
    let rep_def_cfg = "            relay_server: None,\n            connections: default_connections(),\n        }\n    }\n}";
    if s.contains(def_cfg) { s = s.replace(def_cfg, rep_def_cfg); }
    else if s.contains(&def_cfg_lf) { s = s.replace(&def_cfg_lf, rep_def_cfg); }

    // 4. Wrap TransferEngine in Arc
    s = s.replace(
        "let engine = TransferEngine::new(4);",
        "let engine = Arc::new(TransferEngine::new(4));"
    );

    // 5. Add tags and generic networking
    s = s.replace(
        "const TAG_SERVER_HELLO: u8 = 0x06;",
        "const TAG_SERVER_HELLO: u8 = 0x06;\nconst TAG_SESSION_JOIN: u8 = 0x07;"
    );
    s = s.replace(
        "async fn send_tagged(stream: &mut TcpStream, tag: u8, payload: &[u8]) -> Result<()> {",
        "async fn send_tagged<W: tokio::io::AsyncWrite + Unpin>(stream: &mut W, tag: u8, payload: &[u8]) -> Result<()> {"
    );
    s = s.replace(
        "async fn receive_tagged(stream: &mut TcpStream) -> Result<(u8, Vec<u8>)> {",
        "async fn receive_tagged<R: tokio::io::AsyncRead + Unpin>(stream: &mut R) -> Result<(u8, Vec<u8>)> {"
    );

    // 6. Fix try_connect_fallback
    let tcf_start = s.find("async fn try_connect_fallback(").unwrap();
    let tcf_end = s[tcf_start..].find("async fn cmd_send(").unwrap() + tcf_start;
    let old_tcf = &s.clone()[tcf_start..tcf_end];
    s = s.replace(old_tcf, "async fn try_connect_fallback(
    peer_addr: SocketAddr,
) -> Result<TcpStream> {
    tcp::connect(peer_addr).await
}

");

    // 7. Stub cmd_whoami
    let cwa_start = s.find("async fn cmd_whoami(").unwrap();
    let cwa_end = s[cwa_start..].find("async fn cmd_contacts(").unwrap() + cwa_start;
    let old_cwa = &s.clone()[cwa_start..cwa_end];
    s = s.replace(old_cwa, "async fn cmd_whoami(_name: String, _config: &P2pConfig) -> Result<()> {
    anyhow::bail!(\"whoami disabled\")
}

");

    // 8. Stub cmd_relay
    let cr_start = s.find("async fn cmd_relay(").unwrap();
    let cr_end = s[cr_start..].find("async fn cmd_slow_receive(").unwrap() + cr_start;
    let old_cr = &s.clone()[cr_start..cr_end];
    s = s.replace(old_cr, "async fn cmd_relay(_port: u16) -> Result<()> {
    anyhow::bail!(\"relay disabled\")
}

");

    // 9. Fix cmd_listen
    let cl_start = s.find("async fn cmd_listen(").unwrap();
    let cl_end = s[cl_start..].find("async fn cmd_relay(").unwrap() + cl_start;
    let old_cl = &s.clone()[cl_start..cl_end];
    
    let new_listen = "async fn cmd_listen(
    port: u16,
    output_dir: &str,
    _relay: Option<String>,
    config: &P2pConfig,
) -> Result<()> {
    let hostname = hostname::get().unwrap_or_default().to_string_lossy().to_string();
    let mut discovery = DiscoveryService::new(hostname.clone(), port, config.discovery_port).await?;
    discovery.start().await?;
    let resume_manager = Arc::new(TransferResumeManager::new(config.data_dir.join(\"resume\"))?);
    let listener = tokio::net::TcpListener::bind(format!(\"0.0.0.0:{port}\")).await?;
    let (_shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);
    
    let active_transfers = Arc::new(std::sync::Mutex::new(std::collections::HashMap::new()));
    
    loop {
        tokio::select! {
            result = listener.accept() => {
                let (stream, addr) = result?;
                let handler = ReceiverHandler {
                    resume_manager: resume_manager.clone(),
                    output_dir: std::path::PathBuf::from(output_dir),
                    active_transfers: active_transfers.clone(),
                };
                tokio::spawn(async move {
                    let _ = handler.run(stream, addr).await;
                });
            }
            _ = shutdown_rx.changed() => break,
        }
    }
    Ok(())
}

";
    s = s.replace(old_cl, new_listen);

    // 10. Update cmd_send signature
    let cs_sig = "async fn cmd_send(\r\n    path: PathBuf,\r\n    peer: String,\r\n    compression: i32,\r\n    chunk_size: usize,\r\n    relay: Option<String>,\r\n    config: &P2pConfig,\r\n) -> Result<()> {";
    let cs_sig_lf = cs_sig.replace("\r\n", "\n");
    let cs_sig_rep = "async fn cmd_send(\n    path: PathBuf,\n    peer: String,\n    compression: i32,\n    chunk_size: usize,\n    _relay: Option<String>,\n    connections: Option<usize>,\n    config: &P2pConfig,\n) -> Result<()> {";
    if s.contains(cs_sig) { s = s.replace(cs_sig, cs_sig_rep); }
    else if s.contains(&cs_sig_lf) { s = s.replace(&cs_sig_lf, cs_sig_rep); }
    
    // 11. Add SharedTransferState struct
    let new_sts = "
struct SharedTransferState {
    file_mutex: Arc<tokio::sync::Mutex<tokio::fs::File>>,
    metadata: TransferMetadata,
    session_id: String,
    resume_manager: Arc<TransferResumeManager>,
    chunks_received: Arc<std::sync::atomic::AtomicU64>,
}
";
    s = s.replace("struct ReceiverHandler {", &format!("{new_sts}\nstruct ReceiverHandler {{\n    active_transfers: Arc<std::sync::Mutex<std::collections::HashMap<String, Arc<SharedTransferState>>>>,"));

    // 12. Fix cmd_send using relay block entirely!
    let cmd_send_stream_start = s.find("    let mut stream = if let Some(peer_addr) = peer_addr_opt {").unwrap();
    let cmd_send_stream_end = s[cmd_send_stream_start..].find("    set_nodelay(&stream);").unwrap() + cmd_send_stream_start;
    let old_cmd_send_stream = &s.clone()[cmd_send_stream_start..cmd_send_stream_end];
    
    let new_stream_block = "    let mut stream = if let Some(peer_addr) = peer_addr_opt {
        match try_connect_fallback(peer_addr).await {
            Ok(s) => s,
            Err(e) => anyhow::bail!(\"{e}\"),
        }
    } else {
        anyhow::bail!(\"Cannot resolve peer\");
    };\n";
    s = s.replace(old_cmd_send_stream, new_stream_block);

    fs::write("desktop/p2ptransfer-cli/src/main.rs", s).unwrap();
}