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
use tokio::sync::{oneshot, RwLock, watch};
use std::future::Future;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use std::pin::Pin;
use tauri::Manager;

/// Our application-level protocol negotiation identifier.
pub const ALPN: &[u8] = b"p2ptransfer/1";

/// Pause flags: maps request_id → PauseFlag used to pause/resume/cancel a transfer.
/// Entries are removed when the transfer completes or errors.
pub static TRANSFER_PAUSE_FLAGS: Lazy<DashMap<String, Arc<PauseFlag>>> =
    Lazy::new(DashMap::new);

/// A pause/cancel flag backed by tokio watch channels.
///
/// Using `watch` instead of `Notify` eliminates the lost-wakeup race:
/// if `resume()` fires before `wait_if_paused()` starts waiting, the
/// receiver will immediately observe the current `false` value and return.
pub struct PauseFlag {
    /// pause_tx: current value is `true` when paused, `false` when running.
    pause_tx: watch::Sender<bool>,
    /// cancel_tx: current value is `true` once cancelled.
    cancel_tx: watch::Sender<bool>,
}

impl PauseFlag {
    pub fn new() -> Arc<Self> {
        let (pause_tx, _) = watch::channel(false);
        let (cancel_tx, _) = watch::channel(false);
        Arc::new(Self { pause_tx, cancel_tx })
    }

    pub fn pause(&self) {
        let _ = self.pause_tx.send(true);
    }

    pub fn resume(&self) {
        let _ = self.pause_tx.send(false);
    }

    pub fn cancel(&self) {
        let _ = self.cancel_tx.send(true);
    }

    /// Waits until not paused. Returns `true` if the transfer was cancelled.
    ///
    /// Because `watch::Receiver` always holds the latest value, there is no
    /// lost-wakeup: even if `resume()` fired before we start waiting, we
    /// will immediately see `paused == false` and return.
    pub async fn wait_if_paused(&self) -> bool {
        // Take a snapshot of both receivers.
        let mut pause_rx  = self.pause_tx.subscribe();
        let mut cancel_rx = self.cancel_tx.subscribe();

        loop {
            // If already cancelled, return immediately.
            if *cancel_rx.borrow() {
                return true;
            }
            // If not paused, nothing to do.
            if !*pause_rx.borrow() {
                return false;
            }
            // We are currently paused — wait for either state to change.
            tokio::select! {
                biased;
                _ = cancel_rx.changed() => {
                    if *cancel_rx.borrow() {
                        return true;
                    }
                }
                _ = pause_rx.changed() => {
                    // Loop back to re-check both flags.
                }
            }
        }
    }

    /// Waits until cancelled.
    pub async fn wait_for_cancel(&self) {
        let mut cancel_rx = self.cancel_tx.subscribe();
        // If already cancelled, return immediately.
        if *cancel_rx.borrow() {
            return;
        }
        // Otherwise wait for the value to become true.
        loop {
            if cancel_rx.changed().await.is_err() {
                return; // Sender dropped — treat as cancelled.
            }
            if *cancel_rx.borrow() {
                return;
            }
        }
    }
}

#[derive(Clone, Serialize, Debug)]
pub struct ActiveTransferState {
    pub request_id: String,
    pub role: String, // "sender" | "receiver"
    pub status: String,
    pub file_name: String,
    pub total_bytes: u64,
    pub bytes_transferred: u64,
}

/// Registry of currently active transfers, used to rehydrate the UI after F5 refresh.
pub static ACTIVE_TRANSFERS: Lazy<DashMap<String, ActiveTransferState>> = Lazy::new(DashMap::new);

/// Read/write block size for streaming from disk to QUIC.
/// 4 MB: Large enough to amortise per-write syscall overhead on GbE+ links,
/// while small enough that QUIC can start transmitting before the full chunk
/// is buffered.  With 32 channel slots this gives 128 MB in-flight — enough
/// to saturate a 1 Gbps link at ≤100 ms RTT.
const STREAM_CHUNK_SIZE: usize = 4 * 1024 * 1024;

/// Progress events are emitted no more often than this (milliseconds).
/// 33ms = ~30 FPS
const PROGRESS_INTERVAL_MS: u64 = 33;

/// mpsc channel capacity (sender → network / network → disk).
/// 32 × 4 MB = 128 MB of in-flight data.  Same memory budget as before
/// (was 64 × 1 MB = 64 MB), but fewer dequeue operations and larger
/// writes keep the pipe full on GbE.
const CHANNEL_CAPACITY: usize = 32;

// ── Wire types ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferRequest {
    pub request_id: String,
    pub sender_name: String,
    pub sender_node_id: String,
    pub file_name: String,
    pub file_size: u64,
    #[serde(default)]
    pub resume_offset: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferResponse {
    pub accepted: bool,
    #[serde(default)]
    pub resume_offset: u64,
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
    pub speed_bytes_per_sec: f64,
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

    ACTIVE_TRANSFERS.insert(request.request_id.clone(), ActiveTransferState {
        request_id: request.request_id.clone(),
        role: "receiver".to_string(),
        status: "waiting_for_accept".to_string(),
        file_name: request.file_name.clone(),
        total_bytes: request.file_size,
        bytes_transferred: 0,
    });

    // Pre-create the pause flag
    TRANSFER_PAUSE_FLAGS.insert(request.request_id.clone(), PauseFlag::new());

    crate::open_receive_overlay(request.request_id.clone(), app_handle.clone());

    // 3. Wait for frontend response (60-second timeout)
    let accepted = tokio::time::timeout(std::time::Duration::from_secs(60), rx)
        .await
        .unwrap_or(Ok(false))
        .unwrap_or(false);

    // 4. Send response back to sender
    if let Err(e) = write_json(&mut send, &TransferResponse { accepted, resume_offset: 0 }).await {
        tracing::error!(request_id = %request.request_id, "Failed to send TransferResponse: {e:#}");
        emit_transfer_error(&app_handle, &request.request_id, &format!("Failed to send response: {e}"));
        return Err(e);
    }
    
    // Flush the stream immediately so the sender receives the accept signal without QUIC buffering delay.
    if let Err(e) = send.flush().await {
        tracing::error!(request_id = %request.request_id, "Failed to flush TransferResponse: {e:#}");
    }

    if !accepted {
        tracing::info!(request_id = %request.request_id, "Transfer rejected by user");
        ACTIVE_TRANSFERS.remove(&request.request_id);
        return Ok(());
    }

    if let Some(mut state) = ACTIVE_TRANSFERS.get_mut(&request.request_id) {
        state.status = "transferring".to_string();
    }

    // 5. Receive file data
    let start = std::time::Instant::now();
    let out_dir = output_dir.read().await.clone();
    let out_path = PathBuf::from(&out_dir).join(&request.file_name);

    let is_resume = request.resume_offset > 0 && out_path.exists();
    let final_path = if is_resume {
        out_path.clone()
    } else {
        unique_path(&out_path)
    };

    if let Err(e) = std::fs::create_dir_all(final_path.parent().unwrap_or(std::path::Path::new("."))) {
        emit_transfer_error(&app_handle, &request.request_id, &format!("Cannot create output directory: {e}"));
        return Err(e.into());
    }

    let (tx, mut rx_chan) = tokio::sync::mpsc::channel::<bytes::Bytes>(CHANNEL_CAPACITY);
    let final_path_clone = final_path.clone();
    let req_id_for_writer = request.request_id.clone();
    let app_handle_for_writer = app_handle.clone();
    let resume_offset = request.resume_offset;

    let writer_task = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
        use std::io::{Read, Write};
        let mut hasher = blake3::Hasher::new();

        let file = if is_resume {
            // Read and hash existing prefix bytes for integrity verification
            let mut existing = std::fs::File::open(&final_path_clone)?;
            let mut buf = vec![0u8; 1024 * 1024];
            let mut read_len = 0u64;
            while read_len < resume_offset {
                let to_read = std::cmp::min(buf.len() as u64, resume_offset - read_len) as usize;
                let n = existing.read(&mut buf[..to_read])?;
                if n == 0 { break; }
                hasher.update(&buf[..n]);
                read_len += n as u64;
            }

            std::fs::OpenOptions::new()
                .write(true)
                .append(true)
                .open(&final_path_clone)?
        } else {
            std::fs::File::create(&final_path_clone)?
        };

        let mut writer = std::io::BufWriter::with_capacity(STREAM_CHUNK_SIZE, file);

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
    let mut bytes_received: u64 = 0;
    
    // For calculating windowed speed
    let mut last_progress = std::time::Instant::now();
    let mut speed_window_start = std::time::Instant::now();
    let mut bytes_in_window = 0u64;
    let mut current_speed = 0.0;
    
    let progress_interval = std::time::Duration::from_millis(PROGRESS_INTERVAL_MS);

    // Pre-allocate a reusable read buffer to avoid per-iteration heap allocation.
    let mut buf = bytes::BytesMut::with_capacity(STREAM_CHUNK_SIZE);

    let recv_result: anyhow::Result<()> = async {
        loop {
            // Check for pause (and wait if paused)
            let flag_opt = TRANSFER_PAUSE_FLAGS.get(req_id.as_ref()).map(|f| f.clone());
            if let Some(flag) = &flag_opt {
                if flag.wait_if_paused().await {
                    return Err(anyhow::anyhow!("Transfer cancelled by user"));
                }
            }

            // Reset the buffer for reuse (keeps the underlying allocation).
            buf.clear();
            buf.reserve(STREAM_CHUNK_SIZE);
            
            // Wait for data OR a cancel event
            let read_res = if let Some(flag) = &flag_opt {
                tokio::select! {
                    res = recv.read_buf(&mut buf) => res,
                    _ = flag.wait_for_cancel() => return Err(anyhow::anyhow!("Transfer cancelled by user")),
                }
            } else {
                recv.read_buf(&mut buf).await
            };

            match read_res? {
                0 => break,
                n => {
                    // split_to gives ownership of exactly the read bytes; `buf` retains
                    // the remaining capacity for the next iteration.
                    tx.send(buf.split_to(n).freeze()).await.map_err(|_| anyhow::anyhow!("Writer task died"))?;
                    let bytes = n as u64;
                    bytes_received += bytes;
                    bytes_in_window += bytes;

                    // Calculate speed every ~500ms
                    let window_elapsed = speed_window_start.elapsed().as_secs_f64();
                    if window_elapsed >= 0.5 {
                        current_speed = (bytes_in_window as f64) / window_elapsed;
                        bytes_in_window = 0;
                        speed_window_start = std::time::Instant::now();
                    }

                    if last_progress.elapsed() > progress_interval {
                        if let Some(mut state) = ACTIVE_TRANSFERS.get_mut(req_id.as_ref()) {
                            state.bytes_transferred = bytes_received;
                        }
                        let _ = app_handle.emit_all(
                            "transfer-progress",
                            TransferProgressEvent {
                                request_id: req_id.to_string(),
                                bytes_transferred: bytes_received,
                                total: request.file_size,
                                speed_bytes_per_sec: current_speed,
                            },
                        );
                        last_progress = std::time::Instant::now();
                    }
                }
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
        ACTIVE_TRANSFERS.remove(req_id.as_ref());
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

    ACTIVE_TRANSFERS.remove(req_id.as_ref());

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
    target_node: iroh::EndpointAddr,
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

    tracing::info!(request_id = %req_id, target = %target_node.id, "Connecting to peer");

    // 4s connection timeout for responsive retry when receiver is offline.
    let connection = tokio::time::timeout(
        std::time::Duration::from_secs(4),
        endpoint.connect(target_node, ALPN)
    ).await.map_err(|_| {
        let msg = "Connection timed out".to_string();
        tracing::debug!(request_id = %req_id, "{msg}");
        anyhow::anyhow!(msg)
    })?.map_err(|e| {
        let msg = format!("Failed to connect to peer: {e}");
        tracing::debug!(request_id = %req_id, "{msg}");
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
        resume_offset: 0,
    };

    ACTIVE_TRANSFERS.insert(req_id.to_string(), ActiveTransferState {
        request_id: req_id.to_string(),
        role: "sender".to_string(),
        status: "connecting".to_string(),
        file_name: file_name.clone(),
        total_bytes: file_size,
        bytes_transferred: 0,
    });
    
    // Pre-create the pause flag
    TRANSFER_PAUSE_FLAGS.insert(req_id.to_string(), PauseFlag::new());

    write_json(&mut send, &request).await.map_err(|e| {
        let msg = format!("Failed to send TransferRequest: {e}");
        emit_transfer_error(app_handle, &req_id, &msg);
        ACTIVE_TRANSFERS.remove(req_id.as_ref());
        anyhow::anyhow!(msg)
    })?;

    // ── Phase: Waiting for accept ─────────────────────────
    emit_send_status(app_handle, &req_id, "waiting_for_accept");
    tracing::info!(request_id = %req_id, "Waiting for receiver to accept");

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
        anyhow::bail!("REJECTED");
    }

    // ── Phase: Accepted — spawn reader ──────────
    emit_send_status(app_handle, &req_id, "accepted");

    let (data_tx, mut data_rx) = tokio::sync::mpsc::channel::<bytes::Bytes>(CHANNEL_CAPACITY);
    let file_path_clone = file_path.to_path_buf();
    let req_id_for_reader = req_id.clone();
    let app_handle_reader = app_handle.clone();

    let start_offset = response.resume_offset;
    let reader_task = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
        use std::io::{Read, Seek, SeekFrom};
        let mut file = std::fs::File::open(&file_path_clone).map_err(|e| {
            let msg = format!("Cannot open file {:?}: {e}", file_path_clone);
            tracing::error!(request_id = %req_id_for_reader, "{msg}");
            emit_transfer_error_blocking(&app_handle_reader, &req_id_for_reader, &msg);
            anyhow::anyhow!(msg)
        })?;

        let mut hasher = blake3::Hasher::new();
        let mut buf = vec![0u8; STREAM_CHUNK_SIZE];
        let mut processed = 0u64;
        while processed < start_offset {
            let to_read = std::cmp::min(buf.len() as u64, start_offset - processed) as usize;
            let n = file.read(&mut buf[..to_read])?;
            if n == 0 { break; }
            hasher.update(&buf[..n]);
            processed += n as u64;
        }

        file.seek(SeekFrom::Start(start_offset))?;
        let mut reader = std::io::BufReader::with_capacity(STREAM_CHUNK_SIZE, file);

        loop {
            let mut chunk = vec![0u8; STREAM_CHUNK_SIZE];
            let n = reader.read(&mut chunk).map_err(|e| {
                let msg = format!("File read error: {e}");
                tracing::error!(request_id = %req_id_for_reader, "{msg}");
                emit_transfer_error_blocking(&app_handle_reader, &req_id_for_reader, &msg);
                anyhow::anyhow!(msg)
            })?;

            if n == 0 {
                break;
            }
            
            chunk.truncate(n);
            hasher.update(&chunk);

            // Zero-copy conversion from Vec to Bytes
            if data_tx.blocking_send(bytes::Bytes::from(chunk)).is_err() {
                // Receiver dropped — network side failed
                break;
            }
        }

        Ok(hasher.finalize().to_hex().to_string())
    });

    // ── Phase: Transferring ────────────────────────────────
    emit_send_status(app_handle, &req_id, "transferring");
    tracing::info!(request_id = %req_id, file = %file_name, size = file_size, "Streaming file");

    let start = std::time::Instant::now();
    let mut bytes_sent: u64 = 0;
    
    // For calculating windowed speed
    let mut last_progress = std::time::Instant::now();
    let mut speed_window_start = std::time::Instant::now();
    let mut bytes_in_window = 0u64;
    let mut current_speed = 0.0;
    
    let progress_interval = std::time::Duration::from_millis(PROGRESS_INTERVAL_MS);

    // Stream chunks from the reader task to the QUIC send stream.
    // Loop structure: pause/cancel check → dequeue (cancel-interruptible) → write (cancel-interruptible)
    // This ordering ensures that:
    //   • Pause blocks BEFORE dequeuing the next chunk (reader task also backs up naturally)
    //   • Cancel can interrupt both the wait-for-chunk and the write-to-network phases
    loop {
        // ── Step 1: Check pause/cancel BEFORE dequeuing next chunk ─────────
        // wait_if_paused: blocks while paused, returns true only if cancelled.
        let flag_opt = TRANSFER_PAUSE_FLAGS.get(req_id.as_ref()).map(|f| f.clone());
        if let Some(flag) = &flag_opt {
            if flag.wait_if_paused().await {
                let msg = "Transfer cancelled by user";
                emit_transfer_error(app_handle, &req_id, msg);
                TRANSFER_PAUSE_FLAGS.remove(req_id.as_ref());
                return Err(anyhow::anyhow!(msg));
            }
        }

        // ── Step 2: Dequeue next chunk — interruptible by cancel ───────────
        let chunk = if let Some(flag) = &flag_opt {
            tokio::select! {
                biased; // always poll cancel first so it is never starved
                _ = flag.wait_for_cancel() => {
                    let msg = "Transfer cancelled by user";
                    emit_transfer_error(app_handle, &req_id, msg);
                    TRANSFER_PAUSE_FLAGS.remove(req_id.as_ref());
                    return Err(anyhow::anyhow!(msg));
                }
                c = data_rx.recv() => match c {
                    Some(chunk) => chunk,
                    None => break, // reader task finished — all file data has been sent
                },
            }
        } else {
            match data_rx.recv().await {
                Some(c) => c,
                None => break,
            }
        };

        // ── Step 3: Write chunk to QUIC stream — interruptible by cancel ───
        let write_res = if let Some(flag) = &flag_opt {
            tokio::select! {
                biased;
                _ = flag.wait_for_cancel() => {
                    let msg = "Transfer cancelled by user";
                    emit_transfer_error(app_handle, &req_id, msg);
                    TRANSFER_PAUSE_FLAGS.remove(req_id.as_ref());
                    return Err(anyhow::anyhow!(msg));
                }
                res = send.write_all(&chunk) => res,
            }
        } else {
            send.write_all(&chunk).await
        };

        write_res.map_err(|e| {
            let msg = format!("Network write error: {e}");
            tracing::error!(request_id = %req_id, "{msg}");
            emit_transfer_error(app_handle, &req_id, &msg);
            anyhow::anyhow!(msg)
        })?;

        let bytes = chunk.len() as u64;
        bytes_sent += bytes;
        bytes_in_window += bytes;

        // Calculate speed every ~500ms using a sliding window
        let window_elapsed = speed_window_start.elapsed().as_secs_f64();
        if window_elapsed >= 0.5 {
            current_speed = (bytes_in_window as f64) / window_elapsed;
            bytes_in_window = 0;
            speed_window_start = std::time::Instant::now();
        }

        if last_progress.elapsed() > progress_interval {
            if let Some(mut state) = ACTIVE_TRANSFERS.get_mut(req_id.as_ref()) {
                state.bytes_transferred = bytes_sent;
            }
            let _ = app_handle.emit_all(
                "send-progress",
                TransferProgressEvent {
                    request_id: req_id.to_string(),
                    bytes_transferred: bytes_sent,
                    total: file_size,
                    speed_bytes_per_sec: current_speed,
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
    ACTIVE_TRANSFERS.remove(req_id.as_ref());
    Ok(hash)
}

// ── Helper emitters ───────────────────────────────────────

fn emit_send_status(app_handle: &tauri::AppHandle, request_id: &str, status: &str) {
    if let Some(mut state) = ACTIVE_TRANSFERS.get_mut(request_id) {
        state.status = status.to_string();
    }
    let _ = app_handle.emit_all(
        "send-status",
        SendStatusEvent {
            request_id: request_id.to_string(),
            status: status.to_string(),
        },
    );
}

fn emit_transfer_error(app_handle: &tauri::AppHandle, request_id: &str, error: &str) {
    ACTIVE_TRANSFERS.remove(request_id);
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
    ACTIVE_TRANSFERS.remove(request_id);
    let _ = app_handle.emit_all(
        "transfer-error",
        TransferErrorEvent {
            request_id: request_id.to_string(),
            error: error.to_string(),
        },
    );
}
