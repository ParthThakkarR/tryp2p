use std::io::Write;
use tokio::sync::mpsc;
use p2ptransfer_core::network::alpn::{TransferEvent, PendingRequests, TRANSFER_PAUSE_FLAGS};
use p2ptransfer_core::network::tcp_compat::{handle_incoming, send_file_direct_tcp};

/// Test 1: Direct TCP Transfer with Pause and Resume (Sender-side pause)
/// 1. Sender starts sending a 16MB file (4 blocks of 4MB).
/// 2. Sender pauses the transfer.
/// 3. Verify no more progress is made while paused.
/// 4. Sender resumes the transfer.
/// 5. Transfer completes and checksum matches.
#[tokio::test]
async fn test_tcp_pause_resume_sender_side() {
    let dir = tempfile::tempdir().unwrap();
    let src_path = dir.path().join("tcp_pause_src.bin");
    let out_dir = dir.path().join("received");
    std::fs::create_dir_all(&out_dir).unwrap();

    let file_size: usize = 12 * 1024 * 1024; // 12 MB (3 x 4MB blocks)
    let test_data: Vec<u8> = (0..file_size).map(|i| (i % 251) as u8).collect();
    {
        let mut f = std::fs::File::create(&src_path).unwrap();
        f.write_all(&test_data).unwrap();
    }

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let (recv_tx, mut recv_rx) = mpsc::channel::<TransferEvent>(100);
    let pending_requests = PendingRequests::new();
    let pending_clone = pending_requests.clone();
    let out_dir_clone = out_dir.clone();

    // Receiver task
    let receiver_handle = tokio::spawn(async move {
        let (stream, addr) = listener.accept().await.unwrap();
        handle_incoming(stream, addr, out_dir_clone, &recv_tx, pending_clone).await
    });

    let request_id = "test-pause-resume-req-1".to_string();
    let (send_tx, mut send_rx) = mpsc::channel::<TransferEvent>(100);

    // Automatically accept the incoming transfer on receiver
    tokio::spawn(async move {
        while let Some(evt) = recv_rx.recv().await {
            if let TransferEvent::Incoming(inc) = evt {
                pending_requests.respond(&inc.request_id, true).await;
            }
        }
    });

    // Sender task
    let req_id_sender = request_id.clone();
    let src_path_str = src_path.to_str().unwrap().to_string();
    let sender_handle = tokio::spawn(async move {
        send_file_direct_tcp(
            req_id_sender,
            src_path_str,
            "127.0.0.1",
            port,
            &send_tx,
            false,
        ).await
    });

    // Wait until transfer starts (flag is registered)
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Pause the transfer
    if let Some(flag) = TRANSFER_PAUSE_FLAGS.get(&request_id) {
        flag.pause();
    }

    // Wait 200ms and record bytes transferred
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Drain events to see paused state
    let mut last_bytes = 0u64;
    while let Ok(evt) = send_rx.try_recv() {
        if let TransferEvent::Progress(p) = evt {
            last_bytes = p.bytes_transferred;
        }
    }

    // Wait another 200ms — bytes must NOT increase while paused
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let mut bytes_during_pause = last_bytes;
    while let Ok(evt) = send_rx.try_recv() {
        if let TransferEvent::Progress(p) = evt {
            bytes_during_pause = p.bytes_transferred;
        }
    }
    assert_eq!(last_bytes, bytes_during_pause, "No data should be sent while paused");

    // Resume transfer
    if let Some(flag) = TRANSFER_PAUSE_FLAGS.get(&request_id) {
        flag.resume();
    }

    let send_result = sender_handle.await.unwrap();
    assert!(send_result.is_ok(), "Sender should complete successfully after resume: {:?}", send_result.err());

    let recv_result = receiver_handle.await.unwrap();
    assert!(recv_result.is_ok(), "Receiver should complete successfully after resume: {:?}", recv_result.err());

    // Verify received file matches byte-for-byte
    let dst_file = out_dir.join("tcp_pause_src.bin");
    assert!(dst_file.exists(), "Received file must exist");
    let dst_data = std::fs::read(&dst_file).unwrap();
    assert_eq!(test_data.len(), dst_data.len(), "File lengths must match");
    assert_eq!(test_data, dst_data, "File contents must match byte-for-byte");
}

/// Test 2: Direct TCP Transfer Cancel while transferring
#[tokio::test]
async fn test_tcp_cancel_sender_side() {
    let dir = tempfile::tempdir().unwrap();
    let src_path = dir.path().join("tcp_cancel_src.bin");
    let out_dir = dir.path().join("received_cancel");
    std::fs::create_dir_all(&out_dir).unwrap();

    let file_size: usize = 20 * 1024 * 1024; // 20 MB
    let test_data: Vec<u8> = (0..file_size).map(|i| (i % 251) as u8).collect();
    {
        let mut f = std::fs::File::create(&src_path).unwrap();
        f.write_all(&test_data).unwrap();
    }

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let (recv_tx, mut recv_rx) = mpsc::channel::<TransferEvent>(100);
    let pending_requests = PendingRequests::new();
    let pending_clone = pending_requests.clone();
    let out_dir_clone = out_dir.clone();

    let receiver_handle = tokio::spawn(async move {
        let (stream, addr) = listener.accept().await.unwrap();
        handle_incoming(stream, addr, out_dir_clone, &recv_tx, pending_clone).await
    });

    let request_id = "test-cancel-req-1".to_string();
    let (send_tx, _send_rx) = mpsc::channel::<TransferEvent>(100);

    tokio::spawn(async move {
        while let Some(evt) = recv_rx.recv().await {
            if let TransferEvent::Incoming(inc) = evt {
                pending_requests.respond(&inc.request_id, true).await;
            }
        }
    });

    let req_id_sender = request_id.clone();
    let src_path_str = src_path.to_str().unwrap().to_string();
    let sender_handle = tokio::spawn(async move {
        send_file_direct_tcp(
            req_id_sender,
            src_path_str,
            "127.0.0.1",
            port,
            &send_tx,
            false,
        ).await
    });

    // Wait until transfer starts
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;

    // Cancel the transfer from sender side
    if let Some(flag) = TRANSFER_PAUSE_FLAGS.get(&request_id) {
        flag.cancel();
    }

    let send_result = sender_handle.await.unwrap();
    assert!(send_result.is_err(), "Sender must return error on cancellation");
    assert!(
        send_result.unwrap_err().contains("cancelled"),
        "Error message should mention cancellation"
    );

    let recv_result = receiver_handle.await.unwrap();
    assert!(recv_result.is_err(), "Receiver must return error on cancellation");

    // Partial file must be deleted on cancellation
    let partial_file = out_dir.join("tcp_cancel_src.bin");
    assert!(!partial_file.exists(), "Partial file must be cleaned up / removed on cancellation");
}

/// Test 2b: Direct TCP Transfer Cancel from Receiver Side
#[tokio::test]
async fn test_tcp_cancel_receiver_side() {
    let dir = tempfile::tempdir().unwrap();
    let src_path = dir.path().join("tcp_cancel_recv_src.bin");
    let out_dir = dir.path().join("received_cancel_recv");
    std::fs::create_dir_all(&out_dir).unwrap();

    let file_size: usize = 20 * 1024 * 1024; // 20 MB
    let test_data: Vec<u8> = (0..file_size).map(|i| (i % 251) as u8).collect();
    {
        let mut f = std::fs::File::create(&src_path).unwrap();
        f.write_all(&test_data).unwrap();
    }

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let (recv_tx, mut recv_rx) = mpsc::channel::<TransferEvent>(100);
    let pending_requests = PendingRequests::new();
    let pending_clone = pending_requests.clone();
    let out_dir_clone = out_dir.clone();

    let receiver_handle = tokio::spawn(async move {
        let (stream, addr) = listener.accept().await.unwrap();
        handle_incoming(stream, addr, out_dir_clone, &recv_tx, pending_clone).await
    });

    let request_id = "test-cancel-recv-req-1".to_string();
    let (send_tx, _send_rx) = mpsc::channel::<TransferEvent>(100);

    tokio::spawn(async move {
        while let Some(evt) = recv_rx.recv().await {
            if let TransferEvent::Incoming(inc) = evt {
                pending_requests.respond(&inc.request_id, true).await;
            }
        }
    });

    let req_id_sender = request_id.clone();
    let src_path_str = src_path.to_str().unwrap().to_string();
    let sender_handle = tokio::spawn(async move {
        send_file_direct_tcp(
            req_id_sender,
            src_path_str,
            "127.0.0.1",
            port,
            &send_tx,
            false,
        ).await
    });

    // Wait until transfer starts
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;

    // Cancel the transfer from receiver side
    if let Some(flag) = TRANSFER_PAUSE_FLAGS.get(&request_id) {
        flag.cancel();
    }

    let send_result = sender_handle.await.unwrap();
    assert!(send_result.is_err(), "Sender must return error when receiver cancels");

    let recv_result = receiver_handle.await.unwrap();
    assert!(recv_result.is_err(), "Receiver must return error on cancellation");

    // Partial file must be deleted on receiver cancellation
    let partial_file = out_dir.join("tcp_cancel_recv_src.bin");
    assert!(!partial_file.exists(), "Partial file must be cleaned up / removed when receiver cancels");
}

/// Test 3: Receiver-side pause and resume
#[tokio::test]
async fn test_tcp_pause_resume_receiver_side() {
    let dir = tempfile::tempdir().unwrap();
    let src_path = dir.path().join("tcp_recv_pause_src.bin");
    let out_dir = dir.path().join("received_recv_pause");
    std::fs::create_dir_all(&out_dir).unwrap();

    let file_size: usize = 12 * 1024 * 1024; // 12 MB
    let test_data: Vec<u8> = (0..file_size).map(|i| (i % 251) as u8).collect();
    {
        let mut f = std::fs::File::create(&src_path).unwrap();
        f.write_all(&test_data).unwrap();
    }

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let (recv_tx, mut recv_rx) = mpsc::channel::<TransferEvent>(100);
    let pending_requests = PendingRequests::new();
    let pending_clone = pending_requests.clone();
    let out_dir_clone = out_dir.clone();

    let receiver_handle = tokio::spawn(async move {
        let (stream, addr) = listener.accept().await.unwrap();
        handle_incoming(stream, addr, out_dir_clone, &recv_tx, pending_clone).await
    });

    let request_id = "test-recv-pause-req-1".to_string();
    let (send_tx, _send_rx) = mpsc::channel::<TransferEvent>(100);

    tokio::spawn(async move {
        while let Some(evt) = recv_rx.recv().await {
            if let TransferEvent::Incoming(inc) = evt {
                pending_requests.respond(&inc.request_id, true).await;
            }
        }
    });

    let req_id_sender = request_id.clone();
    let src_path_str = src_path.to_str().unwrap().to_string();
    let sender_handle = tokio::spawn(async move {
        send_file_direct_tcp(
            req_id_sender,
            src_path_str,
            "127.0.0.1",
            port,
            &send_tx,
            false,
        ).await
    });

    // Wait until transfer starts
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Pause via receiver flag
    if let Some(flag) = TRANSFER_PAUSE_FLAGS.get(&request_id) {
        flag.pause();
    }

    // Wait while paused
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Resume via receiver flag
    if let Some(flag) = TRANSFER_PAUSE_FLAGS.get(&request_id) {
        flag.resume();
    }

    let send_result = sender_handle.await.unwrap();
    assert!(send_result.is_ok(), "Sender should complete after receiver resume: {:?}", send_result.err());

    let recv_result = receiver_handle.await.unwrap();
    assert!(recv_result.is_ok(), "Receiver should complete after receiver resume: {:?}", recv_result.err());

    let dst_file = out_dir.join("tcp_recv_pause_src.bin");
    assert!(dst_file.exists(), "Received file must exist");
    let dst_data = std::fs::read(&dst_file).unwrap();
    assert_eq!(test_data, dst_data, "File contents must match byte-for-byte");
}
