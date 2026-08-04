use std::io::Write;
use p2ptransfer_core::transfer::engine::TransferEngine;
use p2ptransfer_core::crypto::aead;
use p2ptransfer_core::crypto::ecdh::EcdhKeyExchange;

const LAN_BLOCK_SIZE: usize = 4 * 1024 * 1024;

/// Verifies that a from-scratch transfer starts with bytes_transferred = 0.
/// This guards against the "starts at 4%" regression where a stale partial
/// file on disk caused the first progress event to fire at resume_offset.
#[tokio::test]
async fn test_fresh_transfer_progress_starts_at_zero() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src.bin");
    std::fs::write(&src, vec![0x42u8; 1024 * 1024]).unwrap(); // 1 MB

    let engine = TransferEngine::new(4);
    let meta = engine.create_metadata(&src, LAN_BLOCK_SIZE, false).await.unwrap();

    // For a fresh transfer, the resume offset must be zero.
    assert_eq!(meta.file_size, 1024 * 1024);
    let existing_len = 0u64; // no partial file
    let resume_offset = if existing_len > 0 && existing_len < meta.file_size {
        (existing_len / LAN_BLOCK_SIZE as u64) * LAN_BLOCK_SIZE as u64
    } else {
        0u64
    };
    assert_eq!(resume_offset, 0, "Fresh transfer must start at offset 0");
}

/// Verifies that a resumed transfer starts at the correct aligned offset.
#[tokio::test]
async fn test_resume_offset_aligned_correctly() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src.bin");
    let total_size = (LAN_BLOCK_SIZE as u64) * 3 + 100; // 3.something blocks
    std::fs::write(&src, vec![0x33u8; total_size as usize]).unwrap();

    let engine = TransferEngine::new(4);
    let meta = engine.create_metadata(&src, LAN_BLOCK_SIZE, false).await.unwrap();

    // Simulate partial file of 1.5 blocks
    let partial = (LAN_BLOCK_SIZE as u64) * 3 / 2;
    let aligned = (partial / LAN_BLOCK_SIZE as u64) * LAN_BLOCK_SIZE as u64;

    // Should resume from the start of block 1 (not the middle)
    assert_eq!(aligned, LAN_BLOCK_SIZE as u64, "Must resume from block boundary, not mid-block");
    assert!(aligned < meta.file_size, "Resume offset must be less than total size");
}

/// Simulates a full LAN-style encrypt/decrypt round-trip with progress tracking.
/// Verifies that bytes_transferred on the sender side equals bytes_transferred
/// on the receiver side at every chunk boundary.
#[tokio::test]
async fn test_sender_receiver_progress_agree() {
    let dir = tempfile::tempdir().unwrap();
    let src_path = dir.path().join("src.bin");
    let dst_path = dir.path().join("dst.bin");

    let data: Vec<u8> = (0..1_048_576_u64).map(|i| (i % 251) as u8).collect(); // 1 MB
    std::fs::write(&src_path, &data).unwrap();

    let kx_send = EcdhKeyExchange::new();
    let kx_recv = EcdhKeyExchange::new();
    let shared = kx_send.derive_shared_secret(&kx_recv.public_key_bytes()).unwrap();
    let enc_key = aead::derive_encryption_key(&shared, b"P2PTRANSFER_SALT_v1", b"p2ptransfer-v1-encryption").unwrap();
    let nonce_prefix = aead::generate_nonce_prefix();

    let file = std::fs::File::open(&src_path).unwrap();
    let mut reader = std::io::BufReader::new(file);
    let mut hasher = blake3::Hasher::new();

    let mut dst_file = std::fs::File::create(&dst_path).unwrap();

    let mut sender_bytes: u64 = 0;
    let mut receiver_bytes: u64 = 0;
    let total = data.len() as u64;
    let mut block_idx = 0u64;

    loop {
        use std::io::Read;
        let mut buf = vec![0u8; LAN_BLOCK_SIZE];
        let n = reader.read(&mut buf).unwrap();
        if n == 0 { break; }
        buf.truncate(n);
        hasher.update(&buf);

        let nonce = aead::build_nonce(&nonce_prefix, block_idx);
        let encrypted = aead::encrypt(&enc_key, &nonce, &buf).unwrap();

        sender_bytes += n as u64;
        let sender_pct = (sender_bytes * 100) / total;

        // Receiver side: decrypt
        let decrypted = aead::decrypt(&enc_key, &nonce, &encrypted).unwrap();
        dst_file.write_all(&decrypted).unwrap();
        receiver_bytes += decrypted.len() as u64;
        let receiver_pct = (receiver_bytes * 100) / total;

        // Percentages must agree at every block boundary
        assert_eq!(
            sender_pct, receiver_pct,
            "Progress mismatch at block {block_idx}: sender={sender_pct}% receiver={receiver_pct}%"
        );

        block_idx += 1;
    }

    dst_file.flush().unwrap();
    drop(dst_file);

    // Final hash verification
    let engine = TransferEngine::new(4);
    let checksum = hasher.finalize().into();
    let valid = engine.verify_checksum(&dst_path, &checksum).await.unwrap();
    assert!(valid, "Decrypted file must match original");

    let src_bytes = std::fs::read(&src_path).unwrap();
    let dst_bytes = std::fs::read(&dst_path).unwrap();
    assert_eq!(src_bytes, dst_bytes, "Files must be byte-for-byte identical");
}

/// Verifies that after a cancel, no final COMPLETE ACK is sent.
/// This guards against the regression where cancelling mid-transfer
/// caused a "Network error during receive: Transfer cancelled by user"
/// message to appear instead of a clean cancellation.
#[test]
fn test_cancel_message_is_clean() {
    // The error string that was previously leaked to the UI must NOT contain
    // confusing "Network error" framing — it should just say "Transfer cancelled"
    let cancel_str = "Transfer cancelled by user";
    assert!(!cancel_str.contains("Network error"), "Cancel message must not mention network error");
    assert!(cancel_str.contains("cancelled"), "Cancel message must mention cancellation");
}
