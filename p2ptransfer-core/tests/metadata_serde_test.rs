use p2ptransfer_core::network::tcp;
use p2ptransfer_core::transfer::engine::{TransferEngine, TransferMetadata};
use std::sync::Arc;
use tokio::net::TcpListener;

/// Proof that TransferMetadata serialization and TCP transport work end-to-end.
#[tokio::test]
async fn test_metadata_tcp_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.bin");
    std::fs::write(&path, b"hello world").unwrap();

    let engine = TransferEngine::new(4);
    let meta = engine.create_metadata(&path, 512).await.unwrap();

    // Bind listener and spawn echo handler
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut stream = stream;
        let msg = tcp::receive_message(&mut stream).await.unwrap();
        // Parse as metadata
        let received_meta: TransferMetadata = serde_json::from_slice(&msg).unwrap();
        assert_eq!(received_meta.file_name, "test.bin");
        assert_eq!(received_meta.file_size, 11);
        // Confirm
        tcp::send_message(&mut stream, b"ACCEPT").await.unwrap();
    });

    // Client sends metadata
    let mut stream = tcp::connect(addr).await.unwrap();
    let json = serde_json::to_vec(&meta).unwrap();
    tcp::send_message(&mut stream, &json).await.unwrap();
    let response = tcp::receive_message(&mut stream).await.unwrap();
    assert_eq!(response, b"ACCEPT");

    server.await.unwrap();
}

/// The listen handler from the CLI, tested standalone.
#[tokio::test]
async fn test_listen_handler_accepts_metadata() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.bin");
    std::fs::write(&path, b"test data 12345").unwrap();

    let engine = TransferEngine::new(4);
    let meta = engine.create_metadata(&path, 512).await.unwrap();

    // The same closure used in cmd_listen
    let handler: tcp::MessageHandler = Arc::new(move |data, addr| {
        let m: TransferMetadata = serde_json::from_slice(&data)?;
        tracing::info!("Incoming transfer: {} from {addr}", m.file_name);
        Ok(b"ACCEPT".to_vec())
    });

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        loop {
            let (stream, remote) = listener.accept().await.unwrap();
            let h = handler.clone();
            tokio::spawn(async move {
                let mut stream = stream;
                let mut len_buf = [0u8; 8];
                let _ = stream.read_exact(&mut len_buf).await;
                let msg_len = u64::from_be_bytes(len_buf) as usize;
                let mut payload = vec![0u8; msg_len];
                let _ = stream.read_exact(&mut payload).await;
                if let Ok(response) = h(payload, remote) {
                    let _ = stream
                        .write_all(&(response.len() as u64).to_be_bytes())
                        .await;
                    let _ = stream.write_all(&response).await;
                }
            });
        }
    });

    let mut stream = tcp::connect(addr).await.unwrap();
    let json = serde_json::to_vec(&meta).unwrap();
    tcp::send_message(&mut stream, &json).await.unwrap();
    let response = tcp::receive_message(&mut stream).await.unwrap();
    assert_eq!(response, b"ACCEPT");
}
