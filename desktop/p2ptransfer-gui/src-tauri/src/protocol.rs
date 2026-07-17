//! Custom P2P transfer protocol over iroh QUIC.
//!
//! ALPN: b"p2ptransfer/1"
//!
//! Wire protocol (over a single bidirectional QUIC stream):
//!   Sender → Receiver: JSON TransferRequest  (length-prefixed)
//!   Receiver → Sender: JSON TransferResponse (length-prefixed)
//!   If accepted:
//!     Sender → Receiver: raw file bytes (streamed)
//!     Sender finishes the send side
//!   Receiver computes BLAKE3 hash after receiving all bytes.

use iroh::endpoint::Connection;
use iroh::protocol::{AcceptError, ProtocolHandler};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncWriteExt, AsyncReadExt};
use tokio::sync::{oneshot, RwLock};
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use dashmap::DashMap;
use once_cell::sync::Lazy;
use std::pin::Pin;

/// Our application-level protocol negotiation identifier.
pub const ALPN: &[u8] = b"p2ptransfer/1";

pub static TRANSFER_PAUSE_FLAGS: Lazy<DashMap<String, Arc<AtomicBool>>> = Lazy::new(DashMap::new);

/// Optimal chunk size for I/O and hashing. 
/// 256KB avoids double-copying overhead and fits well in CPU caches.
const STREAM_CHUNK_SIZE: usize = 256 * 1024;

/// Progress events emitted no more often than this.
const PROGRESS_INTERVAL_MS: u64 = 200;

// ── Wire types ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferRequest {
    pub request_id: String,
    pub sender_name: String,
    pub sender_node_id: String,
    pub file_name: String,
    pub file_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferResponse {
    pub accepted: bool,
}

// ── Events emitted to the frontend ─────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct IncomingTransferEvent {
    pub request_id: String,
    pub sender_name: String,
    pub sender_node_id: String,
    pub file_name: String,
    pub file_size: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransferProgressEvent {
    pub request_id: String,
    pub bytes_transferred: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransferCompleteEvent {
    pub request_id: String,
    pub file_path: String,
    pub blake3_hash: String,
    pub elapsed_secs: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransferErrorEvent {
    pub request_id: String,
    pub error: String,
}

// ── Length-delimited framing helpers ─────────────────────────

async fn write_json<T: Serialize>(
    send: &mut iroh::endpoint::SendStream,
    msg: &T,
) -> anyhow::Result<()> {
    let json = serde_json::to_vec(msg)?;
    let len = (json.len() as u32).to_le_bytes();
    send.write_all(&len).await?;
    send.write_all(&json).await?;
    Ok(())
}

async fn read_json<T: for<'de> Deserialize<'de>>(
    recv: &mut iroh::endpoint::RecvStream,
) -> anyhow::Result<T> {
    let mut len_buf = [0u8; 4];
    recv.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > 10 * 1024 * 1024 {
        anyhow::bail!("Message too large: {len} bytes");
    }
    let mut buf = vec![0u8; len];
    recv.read_exact(&mut buf).await?;
    Ok(serde_json::from_slice(&buf)?)
}

// ── Pending request registry ────────────────────────────────

/// Stores pending incoming transfer requests.
/// When the frontend calls `respond_to_transfer`, we resolve the oneshot.
#[derive(Debug, Clone)]
pub struct PendingRequests {
    inner: Arc<RwLock<HashMap<String, oneshot::Sender<bool>>>>,
}

impl PendingRequests {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn register(&self, id: String) -> oneshot::Receiver<bool> {
        let (tx, rx) = oneshot::channel();
        self.inner.write().await.insert(id, tx);
        rx
    }

    pub async fn respond(&self, id: &str, accepted: bool) -> bool {
        if let Some(tx) = self.inner.write().await.remove(id) {
            let _ = tx.send(accepted);
            true
        } else {
            false
        }
    }
}

// ── Protocol handler (receiver side) ────────────────────────

/// Handles incoming QUIC connections on our custom ALPN.
#[derive(Debug, Clone)]
pub struct TransferProtocol {
    pub pending: PendingRequests,
    pub app_handle: tauri::AppHandle,
    pub output_dir: Arc<RwLock<String>>,
}

impl ProtocolHandler for TransferProtocol {
    #[allow(refining_impl_trait)]
    fn accept(&self, connection: Connection) -> Pin<Box<dyn Future<Output = Result<(), AcceptError>> + Send>> {
        let pending = self.pending.clone();
        let app_handle = self.app_handle.clone();
        let output_dir = self.output_dir.clone();

        Box::pin(async move {
            if let Err(e) = handle_incoming_connection(connection, pending, app_handle, output_dir).await {
                tracing::error!("Incoming transfer failed: {e}");
            }
            Ok(())
        })
    }
}

async fn handle_incoming_connection(
    connection: Connection,
    pending: PendingRequests,
    app_handle: tauri::AppHandle,
    output_dir: Arc<RwLock<String>>,
) -> anyhow::Result<()> {
    use tauri::Manager;

    let (mut send, mut recv) = connection.accept_bi().await?;

    // 1. Read TransferRequest
    let request: TransferRequest = read_json(&mut recv).await?;
    tracing::info!(
        "Incoming transfer request: {} ({} bytes) from {}",
        request.file_name,
        request.file_size,
        request.sender_name
    );

    // 2. Register pending request and emit event to frontend
    let rx = pending.register(request.request_id.clone()).await;

    app_handle
        .emit_all(
            "transfer-incoming",
            IncomingTransferEvent {
                request_id: request.request_id.clone(),
                sender_name: request.sender_name.clone(),
                sender_node_id: request.sender_node_id.clone(),
                file_name: request.file_name.clone(),
                file_size: request.file_size,
            },
        )
        .map_err(|e| anyhow::anyhow!("Failed to emit event: {e}"))?;

    // 3. Wait for frontend response (with 60-second timeout)
    let accepted = tokio::time::timeout(std::time::Duration::from_secs(60), rx)
        .await
        .unwrap_or(Ok(false))  // timeout → reject
        .unwrap_or(false);     // channel error → reject

    // 4. Send response
    write_json(&mut send, &TransferResponse { accepted }).await?;

    if !accepted {
        tracing::info!("Transfer rejected by user: {}", request.file_name);
        return Ok(());
    }

    // 5. Receive file data
    let start = std::time::Instant::now();
    let out_dir = output_dir.read().await.clone();
    let out_path = PathBuf::from(&out_dir).join(&request.file_name);

    // Ensure we don't overwrite — add (1), (2), etc.
    let final_path = unique_path(&out_path);
    std::fs::create_dir_all(final_path.parent().unwrap_or(&PathBuf::from(".")))?;

    // PERFORMANCE: Decouple network async from disk blocking writes using mpsc.
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Vec<u8>>(32);
    let final_path_clone = final_path.clone();

    let writer_task = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
        use std::io::Write;
        let mut file = std::fs::File::create(&final_path_clone)?;
        let mut hasher = blake3::Hasher::new();
        while let Some(chunk) = rx.blocking_recv() {
            hasher.update(&chunk);
            file.write_all(&chunk)?;
        }
        file.sync_all()?;
        Ok(hasher.finalize().to_hex().to_string())
    });

    let mut buf = vec![0u8; STREAM_CHUNK_SIZE];
    let mut bytes_received: u64 = 0;
    let mut last_progress = std::time::Instant::now();
    let progress_interval = std::time::Duration::from_millis(PROGRESS_INTERVAL_MS);

    loop {
        if let Some(flag) = TRANSFER_PAUSE_FLAGS.get(&request.request_id) {
            while flag.load(Ordering::Relaxed) {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
        }

        match recv.read(&mut buf).await? {
            Some(n) if n > 0 => {
                tx.send(buf[..n].to_vec()).await?;
                bytes_received += n as u64;

                if last_progress.elapsed() > progress_interval {
                    let _ = app_handle.emit_all(
                        "transfer-progress",
                        TransferProgressEvent {
                            request_id: request.request_id.clone(),
                            bytes_transferred: bytes_received,
                            total: request.file_size,
                        },
                    );
                    last_progress = std::time::Instant::now();
                }
            }
            _ => break,
        }
    }

    drop(tx); // Signal writer thread to stop
    let hash = writer_task.await??;

    let elapsed = start.elapsed().as_secs_f64();
    let speed_mbps = if elapsed > 0.0 {
        (bytes_received as f64 * 8.0) / (elapsed * 1_000_000.0)
    } else {
        0.0
    };

    tracing::info!(
        "Transfer complete: {} ({} bytes, {:.2}s, {:.1} Mbps, BLAKE3: {})",
        final_path.display(),
        bytes_received,
        elapsed,
        speed_mbps,
        hash
    );

    let _ = app_handle.emit_all(
        "transfer-complete",
        TransferCompleteEvent {
            request_id: request.request_id.clone(),
            file_path: final_path.to_string_lossy().to_string(),
            blake3_hash: hash,
            elapsed_secs: elapsed,
        },
    );

    Ok(())
}

/// Generate a unique file path by appending (1), (2), etc. if the file exists.
fn unique_path(path: &std::path::Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let stem = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let ext = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let parent = path.parent().unwrap_or(std::path::Path::new("."));
    for i in 1..1000 {
        let candidate = parent.join(format!("{stem} ({i}){ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    parent.join(format!("{stem} (copy){ext}"))
}

// ── Sender-side function ────────────────────────────────────

/// Connect to a remote peer and send a file.
/// Returns the BLAKE3 hash of the sent file on success.
pub async fn send_file_to_peer(
    request_id: String,
    endpoint: &iroh::Endpoint,
    target_node_id: iroh::PublicKey,
    file_path: &std::path::Path,
    sender_name: &str,
    app_handle: &tauri::AppHandle,
) -> anyhow::Result<String> {
    use tauri::Manager;

    let file_name = file_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let file_size = std::fs::metadata(file_path)?.len();

    // Connect to peer
    tracing::info!("Connecting to peer {} ...", target_node_id);
    let connection = endpoint.connect(target_node_id, ALPN).await?;

    let (mut send, mut recv) = connection.open_bi().await?;

    // Send TransferRequest
    let request = TransferRequest {
        request_id: request_id.clone(),
        sender_name: sender_name.to_string(),
        sender_node_id: endpoint.id().to_string(),
        file_name: file_name.clone(),
        file_size,
    };
    write_json(&mut send, &request).await?;

    // Wait for response (with 90-second timeout for the user to accept)
    let response: TransferResponse = tokio::time::timeout(
        std::time::Duration::from_secs(90),
        read_json(&mut recv),
    )
    .await
    .map_err(|_| anyhow::anyhow!("Receiver did not respond in time"))??;

    if !response.accepted {
        anyhow::bail!("Transfer was rejected by the receiver");
    }

    // PERFORMANCE: Decouple disk blocking reads from network async using mpsc.
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Vec<u8>>(32);
    let file_path_clone = file_path.to_path_buf();
    
    let reader_task = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
        use std::io::Read;
        let mut file = std::fs::File::open(&file_path_clone)?;
        let mut hasher = blake3::Hasher::new();
        let mut buf = vec![0u8; STREAM_CHUNK_SIZE];
        loop {
            let n = file.read(&mut buf)?;
            if n == 0 { break; }
            hasher.update(&buf[..n]);
            if tx.blocking_send(buf[..n].to_vec()).is_err() {
                break;
            }
        }
        Ok(hasher.finalize().to_hex().to_string())
    });

    let start = std::time::Instant::now();
    let mut bytes_sent: u64 = 0;
    let mut last_progress = std::time::Instant::now();
    let progress_interval = std::time::Duration::from_millis(PROGRESS_INTERVAL_MS);

    while let Some(chunk) = rx.recv().await {
        if let Some(flag) = TRANSFER_PAUSE_FLAGS.get(&request_id) {
            while flag.load(Ordering::Relaxed) {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
        }

        send.write_all(&chunk).await?;
        bytes_sent += chunk.len() as u64;

        if last_progress.elapsed() > progress_interval {
            let _ = app_handle.emit_all(
                "send-progress",
                TransferProgressEvent {
                    request_id: request_id.clone(),
                    bytes_transferred: bytes_sent,
                    total: file_size,
                },
            );
            last_progress = std::time::Instant::now();
        }
    }

    // Finish the send side
    send.finish()?;
    let hash = reader_task.await??;
    let elapsed = start.elapsed().as_secs_f64();
    let speed_mbps = if elapsed > 0.0 {
        (bytes_sent as f64 * 8.0) / (elapsed * 1_000_000.0)
    } else {
        0.0
    };

    tracing::info!(
        "Send complete: {} ({} bytes, {:.2}s, {:.1} Mbps, BLAKE3: {})",
        file_name,
        bytes_sent,
        elapsed,
        speed_mbps,
        hash
    );

    Ok(hash)
}
