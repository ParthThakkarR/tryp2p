use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

use super::chunker::Chunker;
use super::hasher;
use super::resume::TransferResumeManager;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransferState {
    Pending,
    Handshaking,
    Negotiating,
    Transferring,
    Paused,
    Completed,
    Failed(String),
    Cancelled,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TransferMetadata {
    pub session_id: String,
    pub file_name: String,
    pub file_path: PathBuf,
    pub file_size: u64,
    pub chunk_size: usize,
    pub total_chunks: u64,
    pub checksum: [u8; 32],
    #[serde(default)]
    pub resume_offset: u64,
    #[serde(default)]
    pub is_resume: bool,
    #[serde(default)]
    pub nonce_prefix: [u8; 4],
}

#[derive(Debug, Clone)]
pub struct TransferProgress {
    pub session_id: String,
    pub bytes_transferred: u64,
    pub _total_bytes: u64,
    pub chunks_completed: u64,
    pub _total_chunks: u64,
    pub speed_bps: f64,
    pub state: TransferState,
}

pub struct TransferEngine {
    max_parallelism: usize,
    resume_manager: Option<Arc<TransferResumeManager>>,
    active_transfers: Arc<RwLock<Vec<TransferProgress>>>,
}

impl TransferEngine {
    pub fn new(max_parallelism: usize) -> Self {
        Self {
            max_parallelism,
            resume_manager: None,
            active_transfers: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn with_resume_manager(mut self, manager: Arc<TransferResumeManager>) -> Self {
        self.resume_manager = Some(manager);
        self
    }

    pub async fn create_metadata(
        &self,
        path: &Path,
        chunk_size: usize,
    ) -> Result<TransferMetadata> {
        let file_size = tokio::fs::metadata(path)
            .await
            .context("Failed to read file metadata")?
            .len();
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".into());
        let chunk_size = chunk_size.max(super::chunker::MIN_CHUNK_SIZE);
        let chunker = Chunker::new(chunk_size, file_size);

        let owned_path = path.to_path_buf();
        let hash_path = owned_path.clone();
        let checksum = tokio::task::spawn_blocking(move || {
            let file = std::fs::File::open(&hash_path)
                .context("Failed to open file for streaming checksum")?;
            hasher::blake3_hash_reader(file)
        })
        .await
        .context("Failed to compute streaming checksum")??;

        Ok(TransferMetadata {
            session_id: uuid::Uuid::new_v4().to_string(),
            file_name,
            file_path: owned_path,
            file_size,
            chunk_size,
            total_chunks: chunker.total_chunks(),
            checksum,
            resume_offset: 0,
            is_resume: false,
            nonce_prefix: [0u8; 4],
        })
    }

    pub async fn prepare_chunk(
        &self,
        path: &Path,
        metadata: &TransferMetadata,
        chunk_index: u64,
    ) -> Result<Vec<u8>> {
        let chunker = Chunker::new(metadata.chunk_size, metadata.file_size);
        let offset = chunker.chunk_offset(chunk_index);
        let length = chunker.chunk_length(chunk_index);

        if length == 0 {
            return Ok(Vec::new());
        }

        let file = tokio::fs::File::open(path)
            .await
            .context("Failed to open file for chunk read")?;
        use tokio::io::AsyncReadExt;

        let mut buf = vec![0u8; length];
        let mut reader = tokio::io::BufReader::new(file);
        use tokio::io::AsyncSeekExt;
        reader.seek(std::io::SeekFrom::Start(offset)).await?;
        reader.read_exact(&mut buf).await?;

        Ok(buf)
    }

    pub async fn process_received_chunk(
        &self,
        path: &Path,
        chunk_index: u64,
        chunk_size: usize,
        data: &[u8],
    ) -> Result<bool> {
        let offset = chunk_index * chunk_size as u64;

        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .context("Failed to create output directory")?;
        }

        let file = tokio::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path)
            .await
            .context("Failed to open file for chunk write")?;

        use tokio::io::AsyncSeekExt;
        use tokio::io::AsyncWriteExt;
        let mut writer = tokio::io::BufWriter::new(file);
        writer.seek(std::io::SeekFrom::Start(offset)).await?;
        writer.write_all(data).await?;
        writer.flush().await?;

        Ok(true)
    }

    pub async fn verify_checksum(&self, path: &Path, expected: &[u8; 32]) -> Result<bool> {
        let owned_path = path.to_path_buf();
        let expected = *expected;
        let actual = tokio::task::spawn_blocking(move || {
            let file = std::fs::File::open(&owned_path)
                .context("Failed to open file for checksum verification")?;
            hasher::blake3_hash_reader(file)
        })
        .await
        .context("Failed to compute checksum for verification")??;
        Ok(actual == expected)
    }

    pub async fn track_progress(
        &self,
        session_id: &str,
        bytes_transferred: u64,
        _total_bytes: u64,
        chunks_completed: u64,
        _total_chunks: u64,
        speed_bps: f64,
    ) {
        let mut transfers = self.active_transfers.write().await;
        if let Some(progress) = transfers.iter_mut().find(|p| p.session_id == session_id) {
            progress.bytes_transferred = bytes_transferred;
            progress.chunks_completed = chunks_completed;
            progress.speed_bps = speed_bps;
        } else {
            transfers.push(TransferProgress {
                session_id: session_id.to_string(),
                bytes_transferred,
                _total_bytes,
                chunks_completed,
                _total_chunks,
                speed_bps,
                state: TransferState::Transferring,
            });
        }
    }

    pub async fn get_progress(&self, session_id: &str) -> Option<TransferProgress> {
        let transfers = self.active_transfers.read().await;
        transfers
            .iter()
            .find(|p| p.session_id == session_id)
            .cloned()
    }

    pub async fn list_active_transfers(&self) -> Vec<TransferProgress> {
        self.active_transfers.read().await.clone()
    }

    pub fn max_parallelism(&self) -> usize {
        self.max_parallelism
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transfer::chunker::MIN_CHUNK_SIZE;
    use std::io::Write;

    fn create_temp_file(size: usize) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_file.bin");
        let mut file = std::fs::File::create(&path).unwrap();
        let data: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
        file.write_all(&data).unwrap();
        (dir, path)
    }

    #[tokio::test]
    async fn test_create_metadata() {
        let (_dir, path) = create_temp_file(1024);
        let engine = TransferEngine::new(4);
        let meta = engine.create_metadata(&path, MIN_CHUNK_SIZE).await.unwrap();

        assert_eq!(meta.file_size, 1024);
        assert_eq!(meta.file_name, "test_file.bin");
        assert_eq!(meta.chunk_size, MIN_CHUNK_SIZE);
        assert_eq!(meta.total_chunks, 1);
    }

    #[tokio::test]
    async fn test_prepare_chunk() {
        let (_dir, path) = create_temp_file(MIN_CHUNK_SIZE);
        let engine = TransferEngine::new(4);
        let meta = engine.create_metadata(&path, MIN_CHUNK_SIZE).await.unwrap();

        let chunk0 = engine.prepare_chunk(&path, &meta, 0).await.unwrap();
        assert_eq!(chunk0.len(), MIN_CHUNK_SIZE);
        assert_eq!(chunk0[0], 0);
        assert_eq!(chunk0[255], 255);

        let chunk1 = engine.prepare_chunk(&path, &meta, 1).await.unwrap();
        assert_eq!(chunk1.len(), 0);
    }

    #[tokio::test]
    async fn test_chunk_write_then_read() {
        let dir = tempfile::tempdir().unwrap();
        let out_path = dir.path().join("received.bin");
        let engine = TransferEngine::new(4);

        let data = vec![0xABu8; 128];
        let written = engine
            .process_received_chunk(&out_path, 0, 1024 * 1024, &data)
            .await
            .unwrap();
        assert!(written);

        let read_back = tokio::fs::read(&out_path).await.unwrap();
        assert_eq!(read_back, data);
    }

    #[tokio::test]
    async fn test_verify_checksum() {
        let (_dir, path) = create_temp_file(256);
        let engine = TransferEngine::new(4);
        let meta = engine.create_metadata(&path, 256).await.unwrap();

        let valid = engine.verify_checksum(&path, &meta.checksum).await.unwrap();
        assert!(valid);

        let bad_checksum = [0xFFu8; 32];
        let invalid = engine.verify_checksum(&path, &bad_checksum).await.unwrap();
        assert!(!invalid);
    }

    #[tokio::test]
    async fn test_track_and_get_progress() {
        let engine = TransferEngine::new(4);
        let session_id = "test-session-123";

        engine
            .track_progress(session_id, 500, 1000, 5, 10, 1_000_000.0)
            .await;

        let progress = engine.get_progress(session_id).await;
        assert!(progress.is_some());
        let p = progress.unwrap();
        assert_eq!(p.session_id, session_id);
        assert_eq!(p.bytes_transferred, 500);
        assert_eq!(p._total_bytes, 1000);
    }

    #[tokio::test]
    async fn test_empty_chunk_prepare() {
        let (_dir, path) = create_temp_file(0);
        let engine = TransferEngine::new(4);
        let meta = engine.create_metadata(&path, 512).await.unwrap();

        let chunk = engine.prepare_chunk(&path, &meta, 0).await.unwrap();
        assert!(chunk.is_empty());
    }
}
