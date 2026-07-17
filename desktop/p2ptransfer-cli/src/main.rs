use anyhow::{Context, Result};
use p2ptransfer_core::compress::detector;
use p2ptransfer_core::crypto::aead;
use p2ptransfer_core::crypto::ecdh::EcdhKeyExchange;
use p2ptransfer_core::network::tcp::{self, DEFAULT_TCP_PORT};

use p2ptransfer_core::p2p::discovery::DiscoveryService;
use p2ptransfer_core::transfer::engine::{TransferEngine, TransferMetadata};
use p2ptransfer_core::transfer::resume::{TransferResumeManager, TransferStatus};
use clap::{Parser, Subcommand};
use indicatif::{HumanBytes, ProgressBar, ProgressStyle};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::io::{AsyncWriteExt, AsyncSeekExt};
use tokio::net::TcpStream;

fn set_nodelay(stream: &TcpStream) {
    let _ = stream.set_nodelay(true);
}
use tracing::{debug, info, warn};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "p2p", version, about = "Next-gen P2P file transfer tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(long, help = "Path to custom config file")]
    config: Option<PathBuf>,

    #[arg(long, help = "Run as headless daemon")]
    daemon: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Send a file or directory to a peer
    Send {
        /// Path to file or directory
        path: PathBuf,
        /// Peer address (IP:port or hostname:port)
        peer: String,
        /// Compression level (1-22, default 10)
        #[arg(long, default_value = "10")]
        compression: i32,
        /// Chunk size in bytes (default 16MB)
        #[arg(long, default_value_t = 16 * 1024 * 1024)]
        chunk_size: usize,
        /// Number of parallel connections
        #[arg(long)]
        connections: Option<usize>,
        /// Relay server address (host:port) for cross-network transfers
        #[arg(long)]
        relay: Option<String>,
    },
    /// Listen for incoming transfers
    Listen {
        /// TCP port to listen on
        #[arg(long, default_value_t = DEFAULT_TCP_PORT)]
        port: u16,
        /// Output directory for received files
        #[arg(long, default_value = ".")]
        output: String,
        /// Relay server address (host:port) for cross-network transfers
        #[arg(long)]
        relay: Option<String>,
    },
    /// Discover peers on local network
    List {
        /// Discovery duration in seconds
        #[arg(long, default_value_t = 10)]
        duration: u64,
    },
    /// Resume an interrupted transfer
    Resume {
        /// Transfer ID to resume
        transfer_id: String,
    },
    /// Show transfer history
    History,
    /// Show connection status
    Status,
    /// Show or modify configuration
    Config {
        /// Display all config values
        #[arg(long)]
        show: bool,
        /// Set a config key=value (e.g., tcp_port=9877)
        #[arg(long, value_name = "KEY=VALUE")]
        set: Option<String>,
        /// Reset config to defaults
        #[arg(long)]
        reset: bool,
    },
    /// Run a relay server for cross-network transfers
    Relay {
        /// Port to listen on
        #[arg(long, default_value_t = 9878)]
        port: u16,
    },
    /// Register a name and detect local/public IP to generate a Peer ID
    Whoami {
        /// Name to register for yourself
        #[arg(long)]
        name: String,
    },
    /// Manage saved contacts/peers
    Contacts {
        #[command(subcommand)]
        action: ContactsAction,
    },
    /// (Test-only) Receive chunks slowly to test backpressure
    #[command(hide = true)]
    SlowReceive {
        /// Output directory for received files
        #[arg(long, default_value = ".")]
        output: String,
        /// Delay per chunk in milliseconds
        #[arg(long, default_value_t = 500)]
        chunk_delay_ms: u64,
    },
}

#[derive(Subcommand)]
enum ContactsAction {
    /// List all saved contacts
    List,
    /// Remove a saved contact
    Remove {
        /// Name of the contact to remove
        name: String,
    },
}


#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct P2pConfig {
    #[serde(default = "default_tcp_port")]
    tcp_port: u16,
    #[serde(default = "default_chunk_size")]
    chunk_size: usize,
    #[serde(default = "default_compression")]
    compression_level: i32,
    #[serde(default = "default_discovery_port")]
    discovery_port: u16,
    #[serde(default = "default_data_dir")]
    data_dir: PathBuf,
    #[serde(default)]
    relay_server: Option<String>,
    #[serde(default = "default_connections")]
    connections: usize,
}

fn default_tcp_port() -> u16 {
    DEFAULT_TCP_PORT
}
fn default_chunk_size() -> usize {
    16 * 1024 * 1024
}
fn default_compression() -> i32 {
    10
}
fn default_discovery_port() -> u16 {
    9876
}
fn default_connections() -> usize { 4 }

fn default_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("p2p")
}

impl Default for P2pConfig {
    fn default() -> Self {
        Self {
            tcp_port: default_tcp_port(),
            chunk_size: default_chunk_size(),
            compression_level: default_compression(),
            discovery_port: default_discovery_port(),
            data_dir: default_data_dir(),
            relay_server: None,
            connections: default_connections(),
        }
    }
}

impl P2pConfig {
    fn load(path: &PathBuf) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;
        let config: P2pConfig = toml::from_str(&content).context("Failed to parse config file")?;
        Ok(config)
    }

    fn save(&self, path: &PathBuf) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::WARN.into()))
        .init();

    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            // If no args on Windows (double-click from Explorer), pause so they can read
            let is_windows = cfg!(target_os = "windows");
            let no_args = std::env::args().len() <= 1;
            if is_windows && no_args {
                println!();
                println!("  ╔═══════════════════════════════════════════════╗");
                println!("  ║           p2ptransfer - P2P File Transfer    ║");
                println!("  ╚═══════════════════════════════════════════════╝");
                println!();
                println!("  Run from a terminal (cmd/powershell):");
                println!("    p2p send <file> <ip:port>    Send a file");
                println!("    p2p listen                   Receive files");
                println!("    p2p list                     Discover peers");
                println!("    p2p --help                   All commands");
                println!();
                println!("  Press Enter to exit...");
                let mut line = String::new();
                let _ = std::io::stdin().read_line(&mut line);
                std::process::exit(0);
            }
            e.exit();
        }
    };

    let config_path = cli.config.clone().unwrap_or_else(get_default_config_path);
    let config = if config_path.exists() {
        P2pConfig::load(&config_path)?
    } else {
        let config = P2pConfig::default();
        config.save(&config_path)?;
        config
    };

    // If --daemon flag is set, run as headless daemon (listen mode)
    if cli.daemon {
        // Daemon mode: listen for transfers in background, suppress console
        let port = config.tcp_port;
        println!("Starting daemon on port {port}...");
        cmd_listen(port, ".", None, &config).await?;
        return Ok(());
    }

    match cli.command {
        Commands::Send {
            path,
            peer,
            compression,
            chunk_size,
            relay,
            connections,
        } => cmd_send(path, peer, compression, chunk_size, relay, connections, &config).await,
        Commands::Listen {
            port,
            output,
            relay,
        } => cmd_listen(port, &output, relay, &config).await,
        Commands::List { duration } => cmd_list(duration, &config).await,
        Commands::Resume { transfer_id } => cmd_resume(transfer_id, &config).await,
        Commands::History => cmd_history(&config).await,
        Commands::Status => cmd_status(&config).await,
        Commands::Relay { port } => cmd_relay(port).await,
        Commands::Whoami { name } => cmd_whoami(name, &config).await,
        Commands::Contacts { action } => cmd_contacts(action, &config).await,
        Commands::SlowReceive {
            output,
            chunk_delay_ms,
        } => cmd_slow_receive(output, chunk_delay_ms, &config).await,
        Commands::Config { show, set, reset } => {
            if reset {
                let default = P2pConfig::default();
                default.save(&config_path)?;
                println!("Config reset to defaults at {}", config_path.display());
                Ok(())
            } else if let Some(kv) = set {
                let mut config = if config_path.exists() {
                    P2pConfig::load(&config_path)?
                } else {
                    P2pConfig::default()
                };
                apply_config_setting(&mut config, &kv)?;
                config.save(&config_path)?;
                println!("Set config: {kv}");
                Ok(())
            } else if show {
                cmd_config_show(&config, &config_path)
            } else {
                println!("Config path: {}", config_path.display());
                println!("Use --show to display, --set KEY=VALUE to modify, --reset for defaults.");
                Ok(())
            }
        }
    }
}

fn get_default_config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("p2p")
        .join("config.toml")
}

fn format_hex(bytes: &[u8; 32]) -> String {
    let mut s = String::with_capacity(64);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

// Wire protocol tags (1 byte, first byte of every frame payload)
const TAG_METADATA: u8 = 0x00;
const TAG_CHUNK: u8 = 0x01;
const TAG_CHUNK_ACK: u8 = 0x02;
const TAG_COMPLETE: u8 = 0x03;
const TAG_ERROR: u8 = 0x04;
const TAG_CLIENT_HELLO: u8 = 0x05;
const TAG_SERVER_HELLO: u8 = 0x06;
const TAG_SESSION_JOIN: u8 = 0x07;

async fn send_tagged<W: tokio::io::AsyncWrite + Unpin>(stream: &mut W, tag: u8, payload: &[u8]) -> Result<()> {
    let mut framed = Vec::with_capacity(1 + payload.len());
    framed.push(tag);
    framed.extend_from_slice(payload);
    tcp::send_message(stream, &framed).await
}

async fn receive_tagged<R: tokio::io::AsyncRead + Unpin>(stream: &mut R) -> Result<(u8, Vec<u8>)> {
    let data = tcp::receive_message(stream).await?;
    if data.is_empty() {
        anyhow::bail!("Empty message");
    }
    let tag = data[0];
    Ok((tag, data[1..].to_vec()))
}

#[allow(clippy::too_many_arguments)]
async fn send_chunk_with_ack(
    stream: &mut TcpStream,
    engine: &TransferEngine,
    path: &Path,
    metadata: &TransferMetadata,
    chunk_index: u64,
    compression: i32,
    enc_key: &[u8; 32],
    nonce_prefix: &[u8; 4],
) -> Result<i64> {
    let chunk_data = engine.prepare_chunk(path, metadata, chunk_index).await?;
    let chunk_len = chunk_data.len() as i64;

    let should_compress = chunk_len >= 64 && !detector::is_likely_compressed(path, &chunk_data);
    let payload = if should_compress && compression > 0 {
        p2ptransfer_core::compress::zstd::compress(&chunk_data, compression)?
    } else {
        chunk_data
    };

    let compressed_flag: u8 = if should_compress && compression > 0 {
        1
    } else {
        0
    };

    let nonce = aead::build_nonce(nonce_prefix, chunk_index);
    let encrypted_payload = aead::encrypt(enc_key, &nonce, &payload)?;

    let mut chunk_frame = Vec::with_capacity(5 + encrypted_payload.len());
    chunk_frame.extend_from_slice(&(chunk_index as u32).to_le_bytes());
    chunk_frame.push(compressed_flag);
    chunk_frame.extend_from_slice(&encrypted_payload);

    send_tagged(stream, TAG_CHUNK, &chunk_frame)
        .await
        .context("Failed to send chunk")?;

    let (tag, ack) = receive_tagged(stream)
        .await
        .context("Failed to receive ACK")?;

    if tag == TAG_ERROR {
        anyhow::bail!("Peer error: {}", String::from_utf8_lossy(&ack));
    }
    if tag != TAG_CHUNK_ACK {
        anyhow::bail!("Expected CHUNK_ACK, got tag={tag}");
    }
    if ack.len() < 4 {
        anyhow::bail!("Malformed CHUNK_ACK (len={})", ack.len());
    }
    let ack_index = u32::from_le_bytes(ack[..4].try_into().unwrap()) as u64;
    if ack_index != chunk_index {
        anyhow::bail!("Chunk index mismatch: sent {chunk_index}, receiver acked {ack_index}");
    }

    Ok(chunk_len)
}

async fn redo_handshake(
    stream: &mut TcpStream,
    enc_key: &mut [u8; 32],
    nonce_prefix: &mut [u8; 4],
) -> Result<()> {
    let kx = EcdhKeyExchange::new();
    let client_pub = kx.public_key_bytes();
    send_tagged(stream, TAG_CLIENT_HELLO, &client_pub).await?;
    let (hello_tag, server_pub_raw) = receive_tagged(stream).await?;
    if hello_tag != TAG_SERVER_HELLO {
        anyhow::bail!("Expected SERVER_HELLO during re-handshake, got tag={hello_tag}");
    }
    let server_pub_bytes: [u8; 32] = server_pub_raw
        .as_slice()
        .try_into()
        .map_err(|_| anyhow::anyhow!("Invalid server public key length"))?;
    let shared_secret = kx.derive_shared_secret(&server_pub_bytes)?;
    let new_key =
        aead::derive_encryption_key(&shared_secret, b"P2PTRANSFER_SALT_v1", b"p2ptransfer-v1-encryption")?;
    *enc_key = new_key;
    *nonce_prefix = aead::generate_nonce_prefix();
    Ok(())
}

async fn try_connect_fallback(
    peer_addr: SocketAddr,
) -> Result<TcpStream> {
    tcp::connect(peer_addr).await
}

async fn cmd_send(
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
    
    // Attempt to resolve contact from DB first
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
        match try_connect_fallback(peer_addr).await {
            Ok(s) => s,
            Err(e) => anyhow::bail!("{e}"),
        }
    } else {
        anyhow::bail!("Cannot resolve peer");
    };
    set_nodelay(&stream);

    // --- ECDH Handshake ---
    println!("Performing ECDH key exchange...");
    let kx = EcdhKeyExchange::new();
    let client_pub = kx.public_key_bytes();
    send_tagged(&mut stream, TAG_CLIENT_HELLO, &client_pub).await?;
    let (hello_tag, server_pub_raw) = receive_tagged(&mut stream).await?;
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
    info!("Key exchange complete");

    let engine = Arc::new(TransferEngine::new(4));
    let mut metadata = engine.create_metadata(&path, chunk_size).await?;
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

    let file_size = metadata.file_size;
    let file_path = path.clone();

    // --- Check for existing partial transfer ---
    let resume_manager = Arc::new(
        TransferResumeManager::new(config.data_dir.join("resume"))
            .context("Failed to initialize resume manager")?,
    );
    let existing = resume_manager.list_transfers()?.into_iter().find(|t| {
        let p = std::path::Path::new(&t.file_path);
        p == path
            && (t.peer_addr == peer
                || t.peer_addr == "127.0.0.1:9877"
                || t.peer_addr.contains(&peer))
            && !matches!(t.status, TransferStatus::Completed | TransferStatus::Failed)
    });

    let (session_id, resume_offset) = if let Some(record) = existing {
        let offset = record.bytes_transferred.max(0) as u64;
        println!(
            "Resuming transfer {} ({} bytes already transferred)",
            &record.id[..8],
            offset
        );
        (record.id.clone(), offset)
    } else {
        let id = resume_manager.create_transfer(
            &peer,
            file_path.to_str().unwrap_or("unknown"),
            file_size as i64,
        )?;
        (id, 0u64)
    };

    if resume_offset > 0 {
        metadata.resume_offset = resume_offset;
        metadata.is_resume = true;
    }

    // --- Tagged handshake: send metadata ---
    let meta_json = serde_json::to_vec(&metadata)?;
    send_tagged(&mut stream, TAG_METADATA, &meta_json).await?;

    let (tag, response) = receive_tagged(&mut stream).await?;
    if tag == TAG_ERROR {
        let err = String::from_utf8_lossy(&response);
        anyhow::bail!("Peer rejected: {err}");
    }
    if tag != TAG_METADATA || response != b"ACCEPT" {
        anyhow::bail!("Peer rejected transfer (unexpected response tag={tag})");
    }

    let start_chunk = (resume_offset / metadata.chunk_size as u64).min(metadata.total_chunks - 1);

    // Initialize progress bar position if resuming
    if resume_offset > 0 {
        pb.inc(resume_offset);
    }

    // --- Signal handling for graceful pause ---
    let paused = Arc::new(AtomicBool::new(false));
    let paused_clone = paused.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        paused_clone.store(true, Ordering::SeqCst);
        eprintln!("\n[Pause requested — finishing current chunk, then saving state...]");
    });

    // --- Send chunks with retry ---
    let mut bytes_sent: i64 = resume_offset as i64;
    for chunk_index in start_chunk..metadata.total_chunks {
        if paused.load(Ordering::SeqCst) {
            println!(
                "\nTransfer paused at chunk {}/{} ({} bytes). Resume with same command.",
                chunk_index,
                metadata.total_chunks,
                HumanBytes(bytes_sent as u64)
            );
            return Ok(());
        }

        let mut last_err = None;
        for attempt in 0..3 {
            if attempt > 0 {
                let delay = Duration::from_secs(1 << attempt); // 2s, 4s backoff
                eprintln!(
                    "Retry {}/3 for chunk {} in {}s",
                    attempt + 1,
                    chunk_index,
                    delay.as_secs()
                );
                tokio::time::sleep(delay).await;
            }

            match send_chunk_with_ack(
                &mut stream,
                &engine,
                &path,
                &metadata,
                chunk_index,
                compression,
                &enc_key,
                &nonce_prefix,
            )
            .await
            {
                Ok(len) => {
                    pb.inc(len as u64);
                    bytes_sent += len;
                    resume_manager.update_progress(&session_id, bytes_sent)?;
                    last_err = None;
                    break;
                }
                Err(e) => {
                    last_err = Some(e);
                    // Try to reconnect on connection drop
                    if attempt < 2 {
                        warn!(
                            "Chunk {chunk_index} failed (attempt {}/3): {last_err:?}. Reconnecting...",
                            attempt + 1
                        );
                        if let Some(peer_addr) = peer_addr_opt {
                            match tcp::connect(peer_addr).await {
                                Ok(new_stream) => {
                                    stream = new_stream;
                                    set_nodelay(&stream);
                                    // Re-do ECDH handshake
                                    if let Err(handshake_err) =
                                        redo_handshake(&mut stream, &mut enc_key, &mut nonce_prefix)
                                            .await
                                    {
                                        warn!("Re-handshake failed: {handshake_err:?}");
                                    }
                                }
                                Err(conn_err) => {
                                    warn!("Reconnect failed: {conn_err:?}");
                                }
                            }
                        } else {
                            warn!("Reconnect not available for relay connections");
                        }
                    }
                }
            }
        }

        if let Some(e) = last_err {
            resume_manager.fail_transfer(&session_id)?;
            anyhow::bail!("Failed to send chunk {chunk_index} after 3 retries: {e}");
        }
    }

    // --- Wait for receiver verification ---
    let (tag, response) = receive_tagged(&mut stream).await?;
    if tag == TAG_COMPLETE {
        let recv_hash = &response[..response.len().min(32)];
        let expected = &metadata.checksum[..];
        if recv_hash != expected {
            resume_manager.fail_transfer(&session_id)?;
            anyhow::bail!("Checksum mismatch: receiver hash differs from sender");
        }
        pb.finish_with_message("Transfer verified");
        resume_manager.complete_transfer(&session_id, &format_hex(&metadata.checksum))?;
        println!("Transfer complete and verified by receiver");
    } else if tag == TAG_ERROR {
        let err = String::from_utf8_lossy(&response);
        resume_manager.fail_transfer(&session_id)?;
        anyhow::bail!("Receiver reported error: {err}");
    } else {
        resume_manager.fail_transfer(&session_id)?;
        anyhow::bail!("Unexpected response after transfer: tag={tag}");
    }

    Ok(())
}

async fn cmd_listen(
    port: u16,
    output_dir: &str,
    _relay: Option<String>,
    config: &P2pConfig,
) -> Result<()> {
    let hostname = hostname::get().unwrap_or_default().to_string_lossy().to_string();
    let mut discovery = DiscoveryService::new(hostname.clone(), port, config.discovery_port).await?;
    discovery.start().await?;
    let resume_manager = Arc::new(TransferResumeManager::new(config.data_dir.join("resume"))?);
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
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

async fn cmd_relay(_port: u16) -> Result<()> {
    anyhow::bail!("relay disabled")
}

async fn cmd_slow_receive(output: String, chunk_delay_ms: u64, config: &P2pConfig) -> Result<()> {
    let resume_manager = Arc::new(
        TransferResumeManager::new(config.data_dir.join("resume"))
            .context("Failed to initialize resume manager")?,
    );

    let listener = TcpListener::bind(format!("0.0.0.0:{}", DEFAULT_TCP_PORT))
        .await
        .context("Failed to bind TCP listener")?;
    println!(
        "Slow receiver listening on port {DEFAULT_TCP_PORT} ({}ms delay per chunk)",
        chunk_delay_ms
    );

    let (mut stream, addr) = listener.accept().await?;
    println!("Accepted connection from {addr}");
    let output_dir = PathBuf::from(&output);

    // --- ECDH Handshake ---
    let (tag, client_pub_raw) = receive_tagged(&mut stream).await?;
    if tag != TAG_CLIENT_HELLO {
        send_tagged(&mut stream, TAG_ERROR, b"Expected CLIENT_HELLO").await?;
        anyhow::bail!("Expected CLIENT_HELLO, got tag={tag}");
    }
    let client_pub_bytes: [u8; 32] = client_pub_raw
        .as_slice()
        .try_into()
        .map_err(|_| anyhow::anyhow!("Invalid client public key length"))?;
    let kx = EcdhKeyExchange::new();
    let server_pub = kx.public_key_bytes();
    send_tagged(&mut stream, TAG_SERVER_HELLO, &server_pub).await?;
    let shared_secret = kx.derive_shared_secret(&client_pub_bytes)?;
    let enc_key =
        aead::derive_encryption_key(&shared_secret, b"P2PTRANSFER_SALT_v1", b"p2ptransfer-v1-encryption")?;

    // --- Read metadata ---
    let (tag, payload) = receive_tagged(&mut stream).await?;
    if tag != TAG_METADATA {
        send_tagged(&mut stream, TAG_ERROR, b"Expected METADATA").await?;
        anyhow::bail!("Expected METADATA, got tag={tag}");
    }
    let metadata: TransferMetadata = serde_json::from_slice(&payload)?;
    let nonce_prefix = metadata.nonce_prefix;
    info!("Slow-receive: {} from {addr}", metadata.file_name);

    send_tagged(&mut stream, TAG_METADATA, b"ACCEPT").await?;

    let output_path = output_dir.join(&metadata.file_name);
    resume_manager.create_transfer(
        &addr.to_string(),
        &output_path.to_string_lossy(),
        metadata.file_size as i64,
    )?;

    let engine = Arc::new(TransferEngine::new(4));

    // --- Receive chunks with delay ---
    for chunk_index in 0..metadata.total_chunks {
        tokio::time::sleep(Duration::from_millis(chunk_delay_ms)).await;

        let (tag, payload) = receive_tagged(&mut stream).await?;
        if tag == TAG_ERROR {
            anyhow::bail!("Peer sent error");
        }
        if tag != TAG_CHUNK {
            anyhow::bail!("Expected CHUNK, got tag={tag}");
        }
        if payload.len() < 5 {
            anyhow::bail!("Malformed chunk");
        }

        let recv_index = u32::from_le_bytes(payload[..4].try_into().unwrap()) as u64;
        if recv_index != chunk_index {
            anyhow::bail!("Chunk index mismatch: expected {chunk_index}, got {recv_index}");
        }

        let compressed = payload[4] != 0;
        let nonce = aead::build_nonce(&nonce_prefix, chunk_index);
        let decrypted_payload = aead::decrypt(&enc_key, &nonce, &payload[5..])
            .context("Chunk decryption failed (bad tag or corrupted)")?;
        let chunk_data = if compressed {
            p2ptransfer_core::compress::zstd::decompress(&decrypted_payload)?
        } else {
            decrypted_payload
        };

        engine
            .process_received_chunk(&output_path, chunk_index, metadata.chunk_size, &chunk_data)
            .await?;

        let mut ack = Vec::with_capacity(4);
        ack.extend_from_slice(&(chunk_index as u32).to_le_bytes());
        send_tagged(&mut stream, TAG_CHUNK_ACK, &ack).await?;

        if chunk_index % 10 == 0 {
            println!(
                "Slow-received chunk {}/{}",
                chunk_index + 1,
                metadata.total_chunks
            );
        }
    }

    // Verify
    let hash_match = engine
        .verify_checksum(&output_path, &metadata.checksum)
        .await?;
    if hash_match {
        send_tagged(&mut stream, TAG_COMPLETE, &metadata.checksum).await?;
        println!("Slow receive complete — checksum OK");
    } else {
        send_tagged(&mut stream, TAG_ERROR, b"Checksum mismatch").await?;
        anyhow::bail!("Checksum mismatch");
    }

    Ok(())
}


struct SharedTransferState {
    file_mutex: Arc<tokio::sync::Mutex<tokio::fs::File>>,
    metadata: TransferMetadata,
    session_id: String,
    resume_manager: Arc<TransferResumeManager>,
    chunks_received: Arc<std::sync::atomic::AtomicU64>,
}

struct ReceiverHandler {
    active_transfers: Arc<std::sync::Mutex<std::collections::HashMap<String, Arc<SharedTransferState>>>>,
    resume_manager: Arc<TransferResumeManager>,
    output_dir: PathBuf,
}

impl ReceiverHandler {
    async fn run(self, mut stream: TcpStream, addr: SocketAddr) -> Result<()> {
        set_nodelay(&stream);
        // --- Step 0: ECDH Handshake ---
        let (tag, client_pub_raw) = receive_tagged(&mut stream).await?;
        if tag == TAG_ERROR {
            return Ok(());
        }
        if tag != TAG_CLIENT_HELLO {
            send_tagged(&mut stream, TAG_ERROR, b"Expected CLIENT_HELLO").await?;
            anyhow::bail!("Expected CLIENT_HELLO, got tag={tag}");
        }
        let client_pub_bytes: [u8; 32] = client_pub_raw.as_slice().try_into().map_err(|_| anyhow::anyhow!("Invalid key"))?;
        let kx = EcdhKeyExchange::new();
        let server_pub = kx.public_key_bytes();
        send_tagged(&mut stream, TAG_SERVER_HELLO, &server_pub).await?;
        let shared_secret = kx.derive_shared_secret(&client_pub_bytes)?;
        let enc_key = aead::derive_encryption_key(&shared_secret, b"P2PTRANSFER_SALT_v1", b"p2ptransfer-v1-encryption")?;

        // --- Step 1: Read metadata or session join ---
        let (tag, payload) = receive_tagged(&mut stream).await?;
        if tag == TAG_ERROR { return Ok(()); }

        let shared_state;
        if tag == TAG_SESSION_JOIN {
            let join_session_id = String::from_utf8_lossy(&payload).to_string();
            info!("Incoming auxiliary connection for session {} from {addr}", join_session_id);
            let state_opt = {
                let map = self.active_transfers.lock().unwrap();
                map.get(&join_session_id).cloned()
            };
            if let Some(s) = state_opt {
                shared_state = s;
                send_tagged(&mut stream, TAG_METADATA, b"ACCEPT").await?;
            } else {
                send_tagged(&mut stream, TAG_ERROR, b"Unknown session").await?;
                anyhow::bail!("Unknown session_id for TAG_SESSION_JOIN");
            }
        } else if tag == TAG_METADATA {
            let metadata: TransferMetadata = serde_json::from_slice(&payload).context("Failed to parse TransferMetadata")?;
            info!("Incoming transfer: {} from {addr}", metadata.file_name);
            let output_path = self.output_dir.join(&metadata.file_name);
            if let Some(parent) = output_path.parent() { tokio::fs::create_dir_all(parent).await?; }
            
            let mut file = tokio::fs::OpenOptions::new()
                .create(true).truncate(false).write(true)
                .open(&output_path).await.context("Failed to open output file")?;
            file.set_len(metadata.file_size).await?;
            let file_mutex = Arc::new(tokio::sync::Mutex::new(file));

            self.resume_manager.create_transfer(&addr.to_string(), &output_path.to_string_lossy(), metadata.file_size as i64)?;

            let start_chunk = if metadata.is_resume {
                (metadata.resume_offset / metadata.chunk_size as u64).min(metadata.total_chunks - 1)
            } else {
                0
            };

            let state = Arc::new(SharedTransferState {
                file_mutex,
                metadata: metadata.clone(),
                session_id: metadata.session_id.clone(),
                resume_manager: self.resume_manager.clone(),
                chunks_received: Arc::new(std::sync::atomic::AtomicU64::new(start_chunk)),
            });

            {
                let mut map = self.active_transfers.lock().unwrap();
                map.insert(metadata.session_id.clone(), state.clone());
            }
            shared_state = state;
            send_tagged(&mut stream, TAG_METADATA, b"ACCEPT").await?;
        } else {
            send_tagged(&mut stream, TAG_ERROR, b"Expected METADATA or SESSION_JOIN").await?;
            anyhow::bail!("Expected METADATA or SESSION_JOIN, got tag={tag}");
        }

        let metadata = &shared_state.metadata;
        let file_mutex = &shared_state.file_mutex;
        let nonce_prefix = metadata.nonce_prefix;
        let engine = Arc::new(TransferEngine::new(4));
        let session_id = shared_state.session_id.clone();
        let output_path = self.output_dir.join(&metadata.file_name);

        let (mut rh, mut wh) = stream.into_split();
        let (chunk_tx, mut chunk_rx) = tokio::sync::mpsc::channel(32);
        
        let mut reader_task = tokio::spawn(async move {
            loop {
                match receive_tagged(&mut rh).await {
                    Ok((tag, payload)) => { if chunk_tx.send(Ok((tag, payload))).await.is_err() { break; } }
                    Err(e) => { let _ = chunk_tx.send(Err(e)).await; break; }
                }
            }
            rh
        });

        let mut pipeline_error = None;
        let mut join_set = tokio::task::JoinSet::new();

        loop {
            tokio::select! {
                Some(res) = chunk_rx.recv() => {
                    match res {
                        Ok((tag, payload)) => {
                            if tag == TAG_ERROR { pipeline_error = Some(anyhow::anyhow!("Peer error")); break; }
                            if tag != TAG_CHUNK || payload.len() < 5 { pipeline_error = Some(anyhow::anyhow!("Malformed chunk")); break; }
                            
                            let recv_index = u32::from_le_bytes(payload[..4].try_into().unwrap()) as u64;
                            let compressed = payload[4] != 0;
                            let encrypted_payload = payload[5..].to_vec();
                            let enc_key = enc_key;
                            let nonce_prefix = nonce_prefix;
                            let chunk_size = metadata.chunk_size;
                            let file_clone = file_mutex.clone();
                            
                            join_set.spawn(async move {
                                let chunk_data = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
                                    let nonce = aead::build_nonce(&nonce_prefix, recv_index);
                                    let decrypted = aead::decrypt(&enc_key, &nonce, &encrypted_payload)?;
                                    if compressed {
                                        Ok(p2ptransfer_core::compress::zstd::decompress(&decrypted)?)
                                    } else {
                                        Ok(decrypted)
                                    }
                                }).await??;
                                
                                let offset = recv_index * chunk_size as u64;
                                let mut f = file_clone.lock().await;
                                f.seek(std::io::SeekFrom::Start(offset)).await?;
                                f.write_all(&chunk_data).await?;
                                Ok::<_, anyhow::Error>((recv_index, chunk_data.len()))
                            });
                        }
                        Err(e) => { pipeline_error = Some(e); break; }
                    }
                }
                Some(res) = join_set.join_next(), if !join_set.is_empty() => {
                    match res {
                        Ok(Ok((recv_index, len))) => {
                            let mut ack = Vec::with_capacity(4);
                            ack.extend_from_slice(&(recv_index as u32).to_le_bytes());
                            if let Err(e) = send_tagged(&mut wh, TAG_CHUNK_ACK, &ack).await {
                                pipeline_error = Some(e);
                                break;
                            }
                            
                            let old_count = shared_state.chunks_received.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                            let _ = shared_state.resume_manager.update_progress(&session_id, (old_count + 1) as i64 * metadata.chunk_size as i64);
                            
                            if old_count + 1 == metadata.total_chunks {
                                let mut f = file_mutex.lock().await;
                                f.seek(std::io::SeekFrom::Start(0)).await?;
                                let std_file = f.try_clone().await.expect("Failed to clone").into_std().await;
                                let hash = tokio::task::spawn_blocking(move || {
                                    let mut std_file = std_file;
                                    p2ptransfer_core::transfer::hasher::blake3_hash_reader(&mut std_file)
                                }).await??;
                                
                                if hash != metadata.checksum {
                                    send_tagged(&mut wh, TAG_ERROR, b"Checksum mismatch").await?;
                                    shared_state.resume_manager.fail_transfer(&session_id)?;
                                } else {
                                    send_tagged(&mut wh, TAG_COMPLETE, &metadata.checksum).await?;
                                    shared_state.resume_manager.complete_transfer(&session_id, &format_hex(&metadata.checksum))?;
                                }
                                
                                {
                                    let mut map = self.active_transfers.lock().unwrap();
                                    map.remove(&session_id);
                                }
                                break;
                            }
                        }
                        Ok(Err(e)) => { pipeline_error = Some(e); break; }
                        Err(e) => { pipeline_error = Some(anyhow::anyhow!("Task panic: {}", e)); break; }
                    }
                }
                else => {
                    if join_set.is_empty() {
                        // Channel closed and join set empty
                        break;
                    }
                }
            }
        }

        if let Some(e) = pipeline_error {
            shared_state.resume_manager.fail_transfer(&session_id)?;
            anyhow::bail!("Receiver pipeline broken: {e:?}");
        }

        let mut f = file_mutex.lock().await;
        f.flush().await?;
        drop(f);
        drop(chunk_rx);
        reader_task.abort();
        Ok(())
    }
}


async fn cmd_list(duration: u64, config: &P2pConfig) -> Result<()> {
    let hostname = hostname::get()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let mut discovery =
        DiscoveryService::new(hostname, DEFAULT_TCP_PORT, config.discovery_port).await?;
    discovery.start().await?;

    println!("Discovering peers for {duration}s...");
    tokio::time::sleep(Duration::from_secs(duration)).await;

    let peers = discovery.get_peers().await;
    discovery.stop().await;

    if peers.is_empty() {
        println!("No peers discovered on local network.");
        return Ok(());
    }

    let resume_manager_opt = TransferResumeManager::new(config.data_dir.join("resume")).ok();
    let contacts = resume_manager_opt.and_then(|rm| rm.list_contacts().ok()).unwrap_or_default();

    println!("Found {} peer(s):", peers.len());
    for peer in &peers {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        // Find if this peer's IP matches a known contact
        let peer_ip = peer.socket_addr.ip().to_string();
        let known_name = contacts.iter()
            .find(|c| c.last_known_ip == peer_ip)
            .map(|c| format!(" (Saved as '{}')", c.name))
            .unwrap_or_default();

        println!(
            "    {}{} at {} (v{}, {}s ago)",
            peer.device_name,
            known_name,
            peer.socket_addr,
            peer.p2ptransfer_version,
            now.saturating_sub(peer.last_seen_epoch)
        );
    }

    Ok(())
}

async fn cmd_resume(transfer_id: String, config: &P2pConfig) -> Result<()> {
    let manager = TransferResumeManager::new(config.data_dir.join("resume"))?;

    match manager.get_transfer(&transfer_id)? {
        Some(record) => {
            println!("Resuming transfer {transfer_id}:");
            println!("  File: {}", record.file_path);
            println!("  Peer: {}", record.peer_addr);
            println!(
                "  Progress: {}/{} bytes",
                record.bytes_transferred, record.file_size
            );
            println!("  Status: {:?}", record.status);
            println!(
                "\nResume support is active. Re-run `send` with same file to resume from offset."
            );
        }
        None => {
            anyhow::bail!("Transfer not found: {transfer_id}");
        }
    }

    Ok(())
}

async fn cmd_history(config: &P2pConfig) -> Result<()> {
    let manager = TransferResumeManager::new(config.data_dir.join("resume"))?;
    let transfers = manager.list_transfers()?;

    if transfers.is_empty() {
        println!("No transfer history.");
        return Ok(());
    }

    println!("Transfer history ({} entries):", transfers.len());
    for t in &transfers {
        println!(
            "    {}: {} -> {} ({}, {:?})",
            &t.id[..8.min(t.id.len())],
            t.file_path,
            t.peer_addr,
            HumanBytes(t.file_size as u64),
            t.status
        );
    }

    Ok(())
}

async fn cmd_status(config: &P2pConfig) -> Result<()> {
    let hostname = hostname::get()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    println!("P2P Status:");
    println!("  Device: {hostname}");
    println!("  TCP Port: {}", config.tcp_port);
    println!("  Discovery Port: {}", config.discovery_port);
    println!("  Multicast Group: 224.0.0.251");
    println!("  Compression Level: {}", config.compression_level);
    println!("  Chunk Size: {}", HumanBytes(config.chunk_size as u64));
    println!(
        "  Data Dir: {}",
        config.data_dir.display()
    );

    // Show recent transfers from resume DB
    let resume_dir = config.data_dir.join("resume");
    if resume_dir.join("transfers.db").exists() {
        match TransferResumeManager::new(resume_dir) {
            Ok(manager) => {
                let transfers = manager.list_transfers().unwrap_or_default();
                let active = transfers.iter().filter(|t| {
                    !matches!(t.status, TransferStatus::Completed | TransferStatus::Failed)
                }).count();
                println!("  Active Transfers: {active}");
                println!("  Total Transfers: {}", transfers.len());
            }
            Err(_) => {}
        }
    }

    Ok(())
}

fn apply_config_setting(config: &mut P2pConfig, kv: &str) -> Result<()> {
    let (key, value) = kv.split_once('=').context("Format: KEY=VALUE")?;
    match key.trim() {
        "tcp_port" => {
            config.tcp_port = value.trim().parse().context("tcp_port must be a number")?;
        }
        "chunk_size" => {
            config.chunk_size = value.trim().parse().context("chunk_size must be a number")?;
        }
        "compression_level" => {
            config.compression_level = value.trim().parse().context("compression_level must be a number")?;
            if !(1..=22).contains(&config.compression_level) {
                anyhow::bail!("compression_level must be 1-22");
            }
        }
        "discovery_port" => {
            config.discovery_port = value.trim().parse().context("discovery_port must be a number")?;
        }
        "relay_server" => {
            config.relay_server = Some(value.trim().to_string());
        }
        _ => anyhow::bail!("Unknown config key: {key}. Valid keys: tcp_port, chunk_size, compression_level, discovery_port, relay_server"),
    }
    Ok(())
}

fn cmd_config_show(config: &P2pConfig, path: &Path) -> Result<()> {
    println!("Config file: {}", path.display());
    println!("{config:#?}");
    Ok(())
}

async fn cmd_whoami(_name: String, _config: &P2pConfig) -> Result<()> {
    anyhow::bail!("whoami disabled")
}

async fn cmd_contacts(action: ContactsAction, config: &P2pConfig) -> Result<()> {
    let resume_manager = TransferResumeManager::new(config.data_dir.join("resume"))?;
    
    match action {
        ContactsAction::List => {
            let contacts = resume_manager.list_contacts()?;
            if contacts.is_empty() {
                println!("No saved contacts found.");
                return Ok(());
            }
            println!("{:<15} | {:<10} | {:<20} | {}", "Name", "Peer ID", "Last IP:Port", "Last Seen");
            println!("{:-<15}-+-{:-<10}-+-{:-<20}-+-{:-<15}", "", "", "", "");
            for c in contacts {
                let addr = format!("{}:{}", c.last_known_ip, c.last_known_port);
                let last_seen_dt = chrono::DateTime::<chrono::Utc>::from_timestamp(c.last_seen, 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                    .unwrap_or_else(|| "Unknown".to_string());
                println!("{:<15} | {:<10} | {:<20} | {}", c.name, c.peer_id, addr, last_seen_dt);
            }
        }
        ContactsAction::Remove { name } => {
            resume_manager.delete_contact(&name)?;
            println!("Contact '{}' removed (if it existed).", name);
        }
    }
    Ok(())
}
