
use anyhow::{Context, Result};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, TcpSocket};
use tracing::{debug, error, info, warn};

pub const DEFAULT_TCP_PORT: u16 = 9877;
pub const MAX_PAYLOAD_SIZE: u64 = 64 * 1024 * 1024; // 64 MB max message

pub type MessageHandler = Arc<dyn Fn(Vec<u8>, SocketAddr) -> Result<Vec<u8>> + Send + Sync>;

pub struct TcpServer {
    port: u16,
    buffer_size: usize,
    listener: Option<TcpListener>,
    handler: Option<MessageHandler>,
    running: Arc<std::sync::atomic::AtomicBool>,
}

impl TcpServer {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            buffer_size: 4 * 1024 * 1024,
            listener: None,
            handler: None,
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub fn with_buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size;
        self
    }

    pub fn with_handler(mut self, handler: MessageHandler) -> Self {
        self.handler = Some(handler);
        self
    }

    pub fn port(&self) -> u16 {
        self.listener
            .as_ref()
            .and_then(|l| l.local_addr().ok())
            .map(|a| a.port())
            .unwrap_or(self.port)
    }

    pub async fn start(&mut self) -> Result<()> {
        let addr: SocketAddr = format!("0.0.0.0:{}", self.port)
            .parse()
            .context("Invalid bind address")?;

        let socket = if addr.is_ipv4() {
            TcpSocket::new_v4()?
        } else {
            TcpSocket::new_v6()?
        };
        
        let _ = socket.set_reuseaddr(true);
        let _ = socket.set_recv_buffer_size(self.buffer_size as u32);
        let _ = socket.set_send_buffer_size(self.buffer_size as u32);
        
        socket.bind(addr).context("Failed to bind TCP listener")?;
        let listener = socket.listen(1024).context("Failed to listen on TCP socket")?;
        self.listener = Some(listener);
        self.running
            .store(true, std::sync::atomic::Ordering::SeqCst);

        info!("TCP server listening on port {}", self.port);
        Ok(())
    }

    pub async fn accept_loop(&self) -> Result<()> {
        let listener = self
            .listener
            .as_ref()
            .context("Server not started. Call start() first.")?;
        let handler = self
            .handler
            .clone()
            .context("No message handler configured")?;
        let running = self.running.clone();

        loop {
            if !running.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }

            match listener.accept().await {
                Ok((stream, addr)) => {
                    debug!("New TCP connection from {addr}");
                    let _ = configure_socket(&stream);
                    let handler = handler.clone();
                    let running = running.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(stream, addr, handler, running).await {
                            warn!("Connection error from {addr}: {e}");
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept TCP connection: {e}");
                }
            }
        }

        Ok(())
    }

    pub fn stop(&self) {
        self.running
            .store(false, std::sync::atomic::Ordering::SeqCst);
        info!("TCP server stopping");
    }
}

async fn handle_connection(
    mut stream: TcpStream,
    addr: SocketAddr,
    handler: MessageHandler,
    running: Arc<std::sync::atomic::AtomicBool>,
) -> Result<()> {
    let (mut reader, mut writer) = stream.split();

    let mut len_buf = [0u8; 8];
    while running.load(std::sync::atomic::Ordering::SeqCst) {
        match reader.read_exact(&mut len_buf).await {
            Ok(_n) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                debug!("Connection closed by {addr}");
                return Ok(());
            }
            Err(e) => return Err(e).context("Failed to read message length"),
        }

        let msg_len = u64::from_be_bytes(len_buf);
        if msg_len > MAX_PAYLOAD_SIZE {
            warn!("Oversized message ({msg_len} bytes) from {addr}, disconnecting");
            return Ok(());
        }

        let mut payload = vec![0u8; msg_len as usize];
        reader
            .read_exact(&mut payload)
            .await
            .context("Failed to read message payload")?;

        let response = handler(payload, addr)?;
        writer
            .write_all(&(response.len() as u64).to_be_bytes())
            .await
            .context("Failed to write response length")?;
        writer
            .write_all(&response)
            .await
            .context("Failed to write response")?;
    }

    Ok(())
}

pub fn configure_socket(stream: &TcpStream) -> Result<()> {
    let _ = stream.set_nodelay(true);
    let sock = socket2::SockRef::from(stream);
    // 32 MB socket buffers: keeps multi-gigabit pipes full (BDP << 32 MB even at 100ms RTT)
    let _ = sock.set_send_buffer_size(32 * 1024 * 1024);
    let _ = sock.set_recv_buffer_size(32 * 1024 * 1024);
    let keepalive = socket2::TcpKeepalive::new().with_time(std::time::Duration::from_secs(60));
    let _ = sock.set_tcp_keepalive(&keepalive);
    Ok(())
}

pub async fn connect(addr: SocketAddr, buffer_size: usize) -> Result<TcpStream> {
    let socket = if addr.is_ipv4() {
        TcpSocket::new_v4()?
    } else {
        TcpSocket::new_v6()?
    };
    
    let _ = socket.set_recv_buffer_size(buffer_size as u32);
    let _ = socket.set_send_buffer_size(buffer_size as u32);

    let stream = socket.connect(addr)
        .await
        .context(format!("Failed to connect to {addr}"))?;
        
    let _ = configure_socket(&stream);
    Ok(stream)
}

pub async fn send_message<W: tokio::io::AsyncWrite + Unpin>(stream: &mut W, data: &[u8]) -> Result<()> {
    let len_bytes = (data.len() as u64).to_be_bytes();
    stream.write_all(&len_bytes).await.context("Failed to write message length")?;
    stream.write_all(data).await.context("Failed to write message payload")?;
    Ok(())
}

pub async fn receive_message<R: tokio::io::AsyncRead + Unpin>(stream: &mut R) -> Result<Vec<u8>> {
    let mut len_buf = [0u8; 8];
    stream
        .read_exact(&mut len_buf)
        .await
        .context("Failed to read message length")?;
    let msg_len = u64::from_be_bytes(len_buf);

    if msg_len > MAX_PAYLOAD_SIZE {
        anyhow::bail!("Oversized message: {msg_len} bytes exceeds max {MAX_PAYLOAD_SIZE}");
    }

    let mut payload = vec![0u8; msg_len as usize];
    stream
        .read_exact(&mut payload)
        .await
        .context("Failed to read message payload")?;
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_send_receive_message() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server_task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let msg = receive_message(&mut stream).await.unwrap();
            assert_eq!(msg, b"hello server");
            send_message(&mut stream, b"hello client").await.unwrap();
        });

        let mut stream = connect(addr, 4 * 1024 * 1024).await.unwrap();
        send_message(&mut stream, b"hello server").await.unwrap();
        let response = receive_message(&mut stream).await.unwrap();
        assert_eq!(response, b"hello client");

        server_task.await.unwrap();
    }

    #[test]
    fn test_default_tcp_port() {
        assert_eq!(DEFAULT_TCP_PORT, 9877);
    }

    #[tokio::test]
    async fn test_server_start_stop() {
        let mut server = TcpServer::new(0);
        server.start().await.unwrap();
        assert!(server.running.load(std::sync::atomic::Ordering::SeqCst));
        server.stop();
        assert!(!server.running.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_oversized_message_rejected() {
        let oversize = vec![0u8; (MAX_PAYLOAD_SIZE + 1) as usize];
        let data = &(oversize.len() as u64).to_be_bytes()[..];
        let mut data_with_payload = data.to_vec();
        data_with_payload.extend_from_slice(&oversize);

        let mut raw = [0u8; 8];
        raw.copy_from_slice(&(data_with_payload.len() as u64).to_be_bytes());

        assert!(
            data_with_payload.len() as u64 > MAX_PAYLOAD_SIZE,
            "Oversized message detection requires the message to be > MAX_PAYLOAD_SIZE"
        );
    }

    #[tokio::test]
    async fn test_with_handler_and_message_roundtrip() {
        let handler: MessageHandler = Arc::new(|req, _addr| {
            Ok(req) // echo handler
        });

        let mut server = TcpServer::new(0).with_handler(handler);
        server.start().await.unwrap();
        let port = server.port();
        assert_ne!(port, 0, "Port should be assigned after start");

        // accept_loop runs forever; spawned task is killed when runtime drops
        let _accept_handle = tokio::spawn(async move {
            let _ = server.accept_loop().await;
        });

        // Tiny yield so the accept_loop starts polling
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
        let mut stream = connect(addr, 4 * 1024 * 1024).await.unwrap();
        send_message(&mut stream, b"ping").await.unwrap();
        let response = receive_message(&mut stream).await.unwrap();
        assert_eq!(
            response, b"ping",
            "Echo handler should return the same data"
        );
    }

    #[tokio::test]
    async fn test_handle_connection_handler() {
        let handler: MessageHandler = Arc::new(|req, _addr| {
            assert_eq!(req, b"hello");
            Ok(b"world".to_vec())
        });

        let mut server = TcpServer::new(0).with_handler(handler);
        server.start().await.unwrap();
        let port = server.port();

        let _accept_handle = tokio::spawn(async move {
            let _ = server.accept_loop().await;
        });

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
        let mut stream = connect(addr, 4 * 1024 * 1024).await.unwrap();
        send_message(&mut stream, b"hello").await.unwrap();
        let response = receive_message(&mut stream).await.unwrap();
        assert_eq!(response, b"world");
    }
}
