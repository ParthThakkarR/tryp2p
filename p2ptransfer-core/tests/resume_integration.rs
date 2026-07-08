use std::io::Write;

use p2ptransfer_core::transfer::engine::TransferEngine;
use p2ptransfer_core::transfer::resume::TransferResumeManager;

/// Simulates a full send→crash→resume cycle at the engine level.
/// Proves chunks are written at the correct offsets and the
/// final reconstructed file is byte-for-byte identical to the source.
#[tokio::test]
async fn test_engine_resume_after_crash() {
    let dir = tempfile::tempdir().unwrap();
    let src_path = dir.path().join("source.bin");
    let dst_path = dir.path().join("received.bin");

    // Create a 2MB source file with deterministic content
    let chunk_size: usize = 512 * 1024; // 512 KB
    let total_size: u64 = 2 * 1024 * 1024; // 2 MB
    {
        let mut f = std::fs::File::create(&src_path).unwrap();
        let data: Vec<u8> = (0..total_size).map(|i| (i % 251) as u8).collect();
        f.write_all(&data).unwrap();
    }

    let engine = TransferEngine::new(4);
    let meta = engine.create_metadata(&src_path, chunk_size).await.unwrap();
    assert_eq!(meta.file_size, total_size);

    // Phase 1: Send first half of chunks, then "crash"
    let crash_at_chunk = meta.total_chunks / 2;
    for i in 0..crash_at_chunk {
        let data = engine.prepare_chunk(&src_path, &meta, i).await.unwrap();
        engine
            .process_received_chunk(&dst_path, i, chunk_size, &data)
            .await
            .unwrap();
    }

    // Verify partial file size
    let partial_size = crash_at_chunk * chunk_size as u64;
    assert_eq!(
        std::fs::metadata(&dst_path).unwrap().len(),
        partial_size,
        "Partial file should be exactly {partial_size} bytes after {crash_at_chunk} chunks"
    );

    // Phase 2: Resume from crash point — send remaining chunks
    for i in crash_at_chunk..meta.total_chunks {
        let data = engine.prepare_chunk(&src_path, &meta, i).await.unwrap();
        engine
            .process_received_chunk(&dst_path, i, chunk_size, &data)
            .await
            .unwrap();
    }

    // Verify checksum
    let valid = engine
        .verify_checksum(&src_path, &meta.checksum)
        .await
        .unwrap();
    assert!(valid, "Source file checksum must be valid against itself");

    let dst_valid = engine
        .verify_checksum(&dst_path, &meta.checksum)
        .await
        .unwrap();
    assert!(
        dst_valid,
        "Resumed file checksum must match source checksum"
    );

    // Byte-for-byte comparison
    let src_bytes = std::fs::read(&src_path).unwrap();
    let dst_bytes = std::fs::read(&dst_path).unwrap();
    assert_eq!(
        src_bytes, dst_bytes,
        "Resumed file must be byte-for-byte identical to source"
    );
}

/// Simulates a full send with SQLite resume tracking:
/// 1. Starts a transfer, records progress
/// 2. "Crashes" (drops the manager)
/// 3. Resumes from last committed offset
/// 4. Verifies final file matches
#[tokio::test]
async fn test_sqlite_resume_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let src_path = dir.path().join("sqlite_resume_src.bin");
    let dst_path = dir.path().join("sqlite_resume_dst.bin");
    let data_dir = dir.path().join("resume_data");

    let total_size: u64 = 1024 * 1024; // 1 MB

    {
        let mut f = std::fs::File::create(&src_path).unwrap();
        let data: Vec<u8> = (0..total_size).map(|i| (i % 199) as u8).collect();
        f.write_all(&data).unwrap();
    }

    let engine = TransferEngine::new(4);
    let meta = engine.create_metadata(&src_path, 256 * 1024).await.unwrap();
    let chunk_size = meta.chunk_size; // Use the clamped chunk size from metadata

    // Phase 1: Send some chunks and record progress
    let session_id;
    let crash_offset: u64;
    {
        let manager = TransferResumeManager::new(data_dir.clone()).unwrap();
        session_id = manager
            .create_transfer(
                "127.0.0.1:9877",
                src_path.to_str().unwrap(),
                total_size as i64,
            )
            .unwrap();

        let send_up_to = meta.total_chunks / 2;
        for i in 0..send_up_to {
            let data = engine.prepare_chunk(&src_path, &meta, i).await.unwrap();
            engine
                .process_received_chunk(&dst_path, i, chunk_size, &data)
                .await
                .unwrap();
            let bytes_so_far = (i + 1) * chunk_size as u64;
            manager
                .update_progress(&session_id, bytes_so_far as i64)
                .unwrap();
        }

        crash_offset = send_up_to * chunk_size as u64;
        // manager drops here — simulates crash
    }

    // Verify partial progress is correct
    let recovered_offset: i64;
    {
        let manager = TransferResumeManager::new(data_dir.clone()).unwrap();
        let record = manager
            .get_transfer(&session_id)
            .unwrap()
            .expect("Transfer record must exist after crash");
        recovered_offset = record.bytes_transferred;
        assert_eq!(
            recovered_offset as u64, crash_offset,
            "Recovered offset must match last committed progress"
        );
    }

    // Phase 2: Resume — send remaining chunks
    let start_chunk = recovered_offset as u64 / chunk_size as u64;
    for i in start_chunk..meta.total_chunks {
        let data = engine.prepare_chunk(&src_path, &meta, i).await.unwrap();
        engine
            .process_received_chunk(&dst_path, i, chunk_size, &data)
            .await
            .unwrap();
    }

    // Debug: verify file sizes
    let dst_len = std::fs::metadata(&dst_path).unwrap().len();
    assert_eq!(
        dst_len, total_size,
        "Final file size must match: expected {total_size}, got {dst_len}"
    );

    // Verify final file
    let valid = engine
        .verify_checksum(&dst_path, &meta.checksum)
        .await
        .unwrap();
    assert!(valid, "SQLite-resumed file checksum must match source");

    let src_bytes = std::fs::read(&src_path).unwrap();
    let dst_bytes = std::fs::read(&dst_path).unwrap();
    assert_eq!(
        src_bytes, dst_bytes,
        "SQLite-resumed file must be identical"
    );
}

/// Test that zero-length chunks are handled correctly in resume scenarios
#[tokio::test]
async fn test_resume_with_empty_chunks() {
    let dir = tempfile::tempdir().unwrap();
    let src_path = dir.path().join("empty_resume_src.bin");
    let dst_path = dir.path().join("empty_resume_dst.bin");

    // Create empty file
    std::fs::write(&src_path, b"").unwrap();

    let engine = TransferEngine::new(4);
    let meta = engine.create_metadata(&src_path, 512 * 1024).await.unwrap();

    // Send the single (empty) chunk
    let data = engine.prepare_chunk(&src_path, &meta, 0).await.unwrap();
    engine
        .process_received_chunk(&dst_path, 0, 512 * 1024, &data)
        .await
        .unwrap();

    let valid = engine
        .verify_checksum(&dst_path, &meta.checksum)
        .await
        .unwrap();
    assert!(valid);
}
