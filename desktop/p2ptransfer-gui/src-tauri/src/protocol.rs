//! Custom P2P transfer protocol over iroh QUIC.
//!
//! ALPN: b"p2ptransfer/1"
//!
//! Wire protocol (over a single bidirectional QUIC stream):
//!   Sender → Receiver: JSON TransferRequest  (length-prefixed u32 LE)
//!   Receiver → Sender: JSON TransferResponse (length-prefixed u32 LE)
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
use tokio::sync::{oneshot, RwLock, Notify};
use std::future::Future;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use std::pin::Pin;
use tauri::Manager;

/// Our application-level protocol negotiation identifier.
pub const ALPN: &[u8] = b"p2ptransfer/1";

/// Pause flags: maps request_id → Notify used to pause/resume a transfer.
/// When a transfer is paused, it waits on the Notify. Resuming calls `notify_waiters()`.
/// Entries are removed when the transfer completes or errors.
pub static TRANSFER_PAUSE_FLAGS: Lazy<DashMap<String, Arc<PauseFlag>>> =
    Lazy::new(DashMap::new);

/// A pause flag backed by an atomic bool + Notify for instant wakeup.
pub struct PauseFlag {
    paused: std::sync::atomic::AtomicBool,
    notify: Notify,
}

impl PauseFlag {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            paused: std::sync::atomic::AtomicBool::new(false),
            notify: Notify::new(),
        })
    }

    pub fn pause(&self) {
        self.paused.store(true, std::sync::atomic::Ordering::Release);
    }

    pub fn resume(&self) {
        self.paused.store(false, std::sync::atomic::Ordering::Release);
        self.notify.notify_waiters();
    }

    /// Waits until not paused. Returns immediately if not paused.
    pub async fn wait_if_paused(&self) {
        loop {
            if !self.paused.load(std::sync::atomic::Ordering::Acquire) {
                return;
            }
            // Wait for a resume signal, then re-check the flag.
            self.notify.notified().await;
        }
    }
}

/// Read size for streaming from disk / network.
/// 256 KB: fits well in L2 cache, balances syscall overhead vs latency.
const STREAM_CHUNK_SIZE: usize = 256 * 1024;

/// Progress events are emitted no more often than this (milliseconds).
const PROGRESS_INTERVAL_MS: u64 = 150;

/// mpsc channel capacity (sender → network / network → disk).
/// 256 × 256 KB = 64 MB of in-flight data.  Keeps the pipe full on GbE.
const CHANNEL_CAPACITY: usize = 256;

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

// ── Tauri events emitted to the frontend ──────────────────

/// Emitted on the sender side at each protocol phase.
#[derive(Debug, Clone, Serialize)]
pub struct SendStatusEvent {
    pub request_id: String,
    /// One of: "connecting" | "waiting_for_accept" | "accepted" | "transferring" | "done"
    pub status: String,
}

/// Emitted when a receiver rejects a transfer.
#[derive(Debug, Clone, Serialize)]
pub struct TransferRejectedEvent {
    pub request_id: String,
}

/// Emitted on any non-fatal or fatal error during a transfer.
#[derive(Debug, Clone, Serialize)]
pub struct TransferErrorEvent {
    pub request_id: String,
    pub error: String,
}

/// Emitted when a new incoming transfer request arrives (receiver side).
#[derive(Debug, Clone, Serialize)]
pub struct IncomingTransferEvent {
    pub request_id: String,
    pub sender_name: String,
    pub sender_node_id: String,
    pub file_name: String,
    pub file_size: u64,
}

/// Transfer progress (bytes received / sent so far).
#[derive(Debug, Clone, Serialize)]
pub struct TransferProgressEvent {
    pub request_id: String,
    pub bytes_transferred: u64,
    pub total: u64,
}

/// Emitted when a transfer completes successfully.
#[derive(Debug, Clone, Serialize)]
pub struct TransferCompleteEvent {
    pub request_id: String,
    pub file_path: String,
    pub blake3_hash: String,
    pub elapsed_secs: f64,
}

// ── Length-delimited framing helpers ─────────────────────

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

// ── Pending request registry ─────────────────────────────

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

    /// Returns a snapshot of all pending request IDs (for reconnect sync).
    pub async fn pending_ids(&self) -> Vec<String> {
        self.inner.read().await.keys().cloned().collect()
    }
}

// ── Protocol handler (receiver side) ─────────────────────

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
                tracing::error!("Incoming transfer failed: {e:#}");
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
    let (mut send, mut recv) = connection.accept_bi().await?;

    // 1. Read TransferRequest
    let request: TransferRequest = read_json(&mut recv).await.map_err(|e| {
        tracing::error!("Failed to read TransferRequest: {e:#}");
        e
    })?;

    tracing::info!(
        request_id = %request.request_id,
        file = %request.file_name,
        size = request.file_size,
        from = %request.sender_name,
        "Incoming transfer request"
    );

    // 2. Register pending + emit event to frontend
    let rx = pending.register(request.request_id.clone()).await;

    if let Err(e) = app_handle.emit_all(
        "transfer-incoming",
        IncomingTransferEvent {
            request_id: request.request_id.clone(),
            sender_name: request.sender_name.clone(),
            sender_node_id: request.sender_node_id.clone(),
            file_name: request.file_name.clone(),
            file_size: request.file_size,
        },
    ) {
        tracing::warn!(request_id = %request.request_id, "Failed to emit transfer-incoming: {e}");
    }

    // 3. Wait for frontend response (60-second timeout)
    let accepted = tokio::time::timeout(std::time::Duration::from_secs(60), rx)
        .await
        .unwrap_or(Ok(false))
        .unwrap_or(false);

    // 4. Send response back to sender
    if let Err(e) = write_json(&mut send, &TransferResponse { accepted }).await {
        tracing::error!(request_id = %request.request_id, "Failed to send TransferResponse: {e:#}");
        emit_transfer_error(&app_handle, &request.request_id, &format!("Failed to send response: {e}"));
        return Err(e);
    }

    if !accepted {
        tracing::info!(request_id = %request.request_id, "Transfer rejected by user");
        return Ok(());
    }

    // 5. Receive file data
    let start = std::time::Instant::now();
    let out_dir = output_dir.read().await.clone();
    let out_path = PathBuf::from(&out_dir).join(&request.file_name);

    // Avoid overwriting existing files by appending (1), (2), etc.
    let final_path = unique_path(&out_path);
    if let Err(e) = std::fs::create_dir_all(final_path.parent().unwrap_or(std::path::Path::new("."))) {
        emit_transfer_error(&app_handle, &request.request_id, &format!("Cannot create output directory: {e}"));
        return Err(e.into());
    }

    // PERFORMANCE: Decouple network receive from disk writes using mpsc.
    // The writer task runs on a blocking thread; the async task reads from the network.
    let (tx, mut rx_chan) = tokio::sync::mpsc::channel::<bytes::Bytes>(CHANNEL_CAPACITY);
    let final_path_clone = final_path.clone();
    let req_id_for_writer = request.request_id.clone();
    let app_handle_for_writer = app_handle.clone();

    let writer_task = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
        use std::io::Write;
        let file = std::fs::File::create(&final_path_clone).map_err(|e| {
            let msg = if e.kind() == std::io::ErrorKind::PermissionDenied {
                format!("Permission denied writing to {:?}", final_path_clone)
            } else {
                format!("Failed to create output file {:?}: {e}", final_path_clone)
            };
            tracing::error!(request_id = %req_id_for_writer, "{msg}");
            emit_transfer_error_blocking(&app_handle_for_writer, &req_id_for_writer, &msg);
            anyhow::anyhow!(msg)
        })?;

        // Use a BufWriter for efficient buffered disk I/O.
        let mut writer = std::io::BufWriter::with_capacity(4 * 1024 * 1024, file);
        let mut hasher = blake3::Hasher::new();

        while let Some(chunk) = rx_chan.blocking_recv() {
            hasher.update(&chunk);
            writer.write_all(&chunk).map_err(|e| {
                let msg = format!("Disk write failed: {e}");
                tracing::error!(request_id = %req_id_for_writer, "{msg}");
                emit_transfer_error_blocking(&app_handle_for_writer, &req_id_for_writer, &msg);
                anyhow::anyhow!(msg)
            })?;
        }

        writer.flush()?;
        let inner = writer.into_inner()?;
        inner.sync_all()?;
        Ok(hasher.finalize().to_hex().to_string())
    });

    // Read from network and forward to writer task.
    // Pre-clone request_id so we don't clone the String inside the loop.
    let req_id = Arc::<str>::from(request.request_id.as_str());
    let mut buf = vec![0u8; STREAM_CHUNK_SIZE];
    let mut bytes_received: u64 = 0;
    let mut last_progress = std::time::Instant::now();
    let progress_interval = std::time::Duration::from_millis(PROGRESS_INTERVAL_MS);

    let recv_result: anyhow::Result<()> = async {
        loop {
            // Check for pause
            if let Some(flag) = TRANSFER_PAUSE_FLAGS.get(req_id.as_ref()) {
                flag.wait_if_paused().await;
            }

            match recv.read(&mut buf).await? {
                Some(n) if n > 0 => {
                    // bytes::Bytes::copy_from_slice avoids an extra alloc on the channel
                    let chunk = bytes::Bytes::copy_from_slice(&buf[..n]);
                    tx.send(chunk).await.map_err(|_| anyhow::anyhow!("Writer task died"))?;
                    bytes_received += n as u64;

                    if last_progress.elapsed() > progress_interval {
                        let _ = app_handle.emit_all(
                            "transfer-progress",
                            TransferProgressEvent {
                                request_id: req_id.to_string(),
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
        Ok(())
    }.await;

    drop(tx); // Signal writer thread to finish

    // Await the writer and collect any disk errors.
    let hash = writer_task.await.map_err(|e| anyhow::anyhow!("Writer task panicked: {e}"))??;

    // Clean up pause flag for this request.
    TRANSFER_PAUSE_FLAGS.remove(req_id.as_ref());

    // Surface any network error that occurred during receive.
    if let Err(e) = recv_result {
        let msg = format!("Network error during receive: {e}");
        tracing::error!(request_id = %req_id, "{msg}");
        emit_transfer_error(&app_handle, &req_id, &msg);
        return Err(e);
    }

    let elapsed = start.elapsed().as_secs_f64();
    let speed_mbps = if elapsed > 0.0 {
        (bytes_received as f64 * 8.0) / (elapsed * 1_000_000.0)
    } else {
        0.0
    };

    tracing::info!(
        request_id = %req_id,
        file = %final_path.display(),
        bytes = bytes_received,
        elapsed_secs = elapsed,
        speed_mbps = speed_mbps,
        blake3 = %hash,
        "Transfer complete"
    );

    let _ = app_handle.emit_all(
        "transfer-complete",
        TransferCompleteEvent {
            request_id: req_id.to_string(),
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

// ── Sender-side function ──────────────────────────────────

/// Connect to a remote peer and send a file.
///
/// Returns the BLAKE3 hash of the sent file on success.
///
/// Emits the following Tauri events:
/// - `send-status`        — phase transitions (connecting, waiting_for_accept, accepted, transferring, done)
/// - `send-progress`      — bytes sent so far
/// - `transfer-rejected`  — receiver declined the transfer
/// - `transfer-error`     — any network or disk error
pub async fn send_file_to_peer(
    request_id: String,
    endpoint: &iroh::Endpoint,
    target_node_id: iroh::PublicKey,
    file_path: &std::path::Path,
    sender_name: &str,
    app_handle: &tauri::AppHandle,
) -> anyhow::Result<String> {
    let req_id = Arc::<str>::from(request_id.as_str());

    // ── Phase: Connecting ────────────────────────────────
    emit_send_status(app_handle, &req_id, "connecting");

    let file_name = file_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let file_size = std::fs::metadata(file_path)
        .map_err(|e| anyhow::anyhow!("Cannot stat file: {e}"))?
        .len();

    tracing::info!(request_id = %req_id, target = %target_node_id, "Connecting to peer");

    let connection = endpoint.connect(target_node_id, ALPN).await.map_err(|e| {
        let msg = format!("Failed to connect to peer: {e}");
        tracing::error!(request_id = %req_id, "{msg}");
        emit_transfer_error(app_handle, &req_id, &msg);
        anyhow::anyhow!(msg)
    })?;

    let (mut send, mut recv) = connection.open_bi().await.map_err(|e| {
        let msg = format!("Failed to open QUIC stream: {e}");
        tracing::error!(request_id = %req_id, "{msg}");
        emit_transfer_error(app_handle, &req_id, &msg);
        anyhow::anyhow!(msg)
    })?;

    // ── Phase: Send TransferRequest ───────────────────────
    let request = TransferRequest {
        request_id: req_id.to_string(),
        sender_name: sender_name.to_string(),
        sender_node_id: endpoint.id().to_string(),
        file_name: file_name.clone(),
        file_size,
    };
    write_json(&mut send, &request).await.map_err(|e| {
        let msg = format!("Failed to send TransferRequest: {e}");
        emit_transfer_error(app_handle, &req_id, &msg);
        anyhow::anyhow!(msg)
    })?;

    // ── Phase: Waiting for accept ─────────────────────────
    emit_send_status(app_handle, &req_id, "waiting_for_accept");
    tracing::info!(request_id = %req_id, "Waiting for receiver to accept");

    // PERFORMANCE: Pre-spawn the reader task NOW, while waiting for the accept.
    // This hides the spawn_blocking scheduling latency and ensures the file is
    // open and buffered the moment we get the accepted signal.
    let (start_tx, start_rx) = tokio::sync::oneshot::channel::<bool>();
    let (data_tx, mut data_rx) = tokio::sync::mpsc::channel::<bytes::Bytes>(CHANNEL_CAPACITY);
    let file_path_clone = file_path.to_path_buf();
    let req_id_for_reader = req_id.clone();
    let app_handle_reader = app_handle.clone();

    let reader_task = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
        // Wait for the "start" signal (true = accepted, false = rejected/timeout)
        match start_rx.blocking_recv() {
            Ok(true) => {}
            _ => {
                // Rejected or cancelled — exit cleanly without error
                return Ok(String::new());
            }
        }

        use std::io::Read;
        let file = std::fs::File::open(&file_path_clone).map_err(|e| {
            let msg = format!("Cannot open file {:?}: {e}", file_path_clone);
            tracing::error!(request_id = %req_id_for_reader, "{msg}");
            emit_transfer_error_blocking(&app_handle_reader, &req_id_for_reader, &msg);
            anyhow::anyhow!(msg)
        })?;

        // BufReader with 4 MB buffer for sequential read performance.
        let mut reader = std::io::BufReader::with_capacity(4 * 1024 * 1024, file);
        let mut hasher = blake3::Hasher::new();
        let mut buf = vec![0u8; STREAM_CHUNK_SIZE];

        loop {
            let n = reader.read(&mut buf).map_err(|e| {
                let msg = format!("File read error: {e}");
                tracing::error!(request_id = %req_id_for_reader, "{msg}");
                emit_transfer_error_blocking(&app_handle_reader, &req_id_for_reader, &msg);
                anyhow::anyhow!(msg)
            })?;

            if n == 0 {
                break;
            }

            hasher.update(&buf[..n]);

            // Send as Bytes (reference-counted; channel send is cheap)
            if data_tx.blocking_send(bytes::Bytes::copy_from_slice(&buf[..n])).is_err() {
                // Receiver dropped — network side failed
                break;
            }
        }

        Ok(hasher.finalize().to_hex().to_string())
    });

    // ── Wait for TransferResponse (up to 90 seconds) ──────
    let response: TransferResponse = tokio::time::timeout(
        std::time::Duration::from_secs(90),
        read_json(&mut recv),
    )
    .await
    .map_err(|_| {
        let msg = "Receiver did not respond within 90 seconds".to_string();
        emit_transfer_error(app_handle, &req_id, &msg);
        anyhow::anyhow!(msg)
    })?
    .map_err(|e| {
        let msg = format!("Failed to read TransferResponse: {e}");
        emit_transfer_error(app_handle, &req_id, &msg);
        anyhow::anyhow!(msg)
    })?;

    if !response.accepted {
        tracing::info!(request_id = %req_id, "Transfer rejected by receiver");
        let _ = app_handle.emit_all("transfer-rejected", TransferRejectedEvent {
            request_id: req_id.to_string(),
        });
        // Await reader task so we don't leak it
        let _ = reader_task.await;
        anyhow::bail!("REJECTED");
    }

    // ── Phase: Accepted — signal reader to start ──────────
    emit_send_status(app_handle, &req_id, "accepted");
    let _ = start_tx.send(true); // Tell the reader to start immediately

    // ── Phase: Transferring ────────────────────────────────
    emit_send_status(app_handle, &req_id, "transferring");
    tracing::info!(request_id = %req_id, file = %file_name, size = file_size, "Streaming file");

    let start = std::time::Instant::now();
    let mut bytes_sent: u64 = 0;
    let mut last_progress = std::time::Instant::now();
    let progress_interval = std::time::Duration::from_millis(PROGRESS_INTERVAL_MS);

    // Stream chunks from the reader task to the QUIC send stream.
    while let Some(chunk) = data_rx.recv().await {
        // Check for pause
        if let Some(flag) = TRANSFER_PAUSE_FLAGS.get(req_id.as_ref()) {
            flag.wait_if_paused().await;
        }

        send.write_all(&chunk).await.map_err(|e| {
            let msg = format!("Network write error: {e}");
            tracing::error!(request_id = %req_id, "{msg}");
            emit_transfer_error(app_handle, &req_id, &msg);
            anyhow::anyhow!(msg)
        })?;

        bytes_sent += chunk.len() as u64;

        if last_progress.elapsed() > progress_interval {
            let _ = app_handle.emit_all(
                "send-progress",
                TransferProgressEvent {
                    request_id: req_id.to_string(),
                    bytes_transferred: bytes_sent,
                    total: file_size,
                },
            );
            last_progress = std::time::Instant::now();
        }
    }

    // Finish the QUIC send stream (signals EOF to receiver)
    send.finish().map_err(|e| {
        let msg = format!("Failed to finish QUIC stream: {e}");
        tracing::error!(request_id = %req_id, "{msg}");
        emit_transfer_error(app_handle, &req_id, &msg);
        anyhow::anyhow!(msg)
    })?;

    // Collect the BLAKE3 hash from the reader task.
    let hash = reader_task
        .await
        .map_err(|e| anyhow::anyhow!("Reader task panicked: {e}"))??;

    // Clean up pause flag.
    TRANSFER_PAUSE_FLAGS.remove(req_id.as_ref());

    let elapsed = start.elapsed().as_secs_f64();
    let speed_mbps = if elapsed > 0.0 {
        (bytes_sent as f64 * 8.0) / (elapsed * 1_000_000.0)
    } else {
        0.0
    };

    tracing::info!(
        request_id = %req_id,
        file = %file_name,
        bytes = bytes_sent,
        elapsed_secs = elapsed,
        speed_mbps = speed_mbps,
        blake3 = %hash,
        "Send complete"
    );

    emit_send_status(app_handle, &req_id, "done");
    Ok(hash)
}

// ── Helper emitters ───────────────────────────────────────

fn emit_send_status(app_handle: &tauri::AppHandle, request_id: &str, status: &str) {
    let _ = app_handle.emit_all(
        "send-status",
        SendStatusEvent {
            request_id: request_id.to_string(),
            status: status.to_string(),
        },
    );
}

fn emit_transfer_error(app_handle: &tauri::AppHandle, request_id: &str, error: &str) {
    let _ = app_handle.emit_all(
        "transfer-error",
        TransferErrorEvent {
            request_id: request_id.to_string(),
            error: error.to_string(),
        },
    );
}

/// Blocking version of `emit_transfer_error` for use inside `spawn_blocking` closures.
fn emit_transfer_error_blocking(app_handle: &tauri::AppHandle, request_id: &str, error: &str) {
    // `emit_all` is thread-safe and does not require an async context.
    let _ = app_handle.emit_all(
        "transfer-error",
        TransferErrorEvent {
            request_id: request_id.to_string(),
            error: error.to_string(),
        },
    );
}
