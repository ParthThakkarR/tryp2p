use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::{TcpListener, TcpSocket, TcpStream};
use tokio::time::timeout;

/// Create a TCP socket configured with SO_REUSEADDR (and SO_REUSEPORT on Unix).
/// This allows multiple sockets (e.g. a listener and an outgoing connection) 
/// to be bound to the exact same local port simultaneously.
pub fn bind_reusable_socket(port: u16) -> std::io::Result<TcpSocket> {
    let socket = TcpSocket::new_v4()?;
    
    // Windows: SO_REUSEADDR allows multiple binds to the same port.
    #[cfg(windows)]
    socket.set_reuseaddr(true)?;
    
    // Unix: We need SO_REUSEADDR and SO_REUSEPORT for simultaneous listen/connect.
    #[cfg(unix)]
    {
        socket.set_reuseaddr(true)?;
        socket.set_reuseport(true)?;
    }
    
    socket.bind(std::net::SocketAddr::new(
        std::net::Ipv4Addr::UNSPECIFIED.into(),
        port,
    ))?;
    Ok(socket)
}

/// Attempt TCP hole punching by simultaneously connecting to a peer
/// while also listening on the same port.
///
/// Returns the established connection or an error.
pub async fn tcp_hole_punch(
    local_port: u16,
    remote_addr: SocketAddr,
    timeout_ms: u64,
) -> Result<TcpStream, String> {
    // Listen on the local port for the peer's incoming connection using a reusable socket
    let socket = bind_reusable_socket(local_port)
        .map_err(|e| format!("Bind reusable socket for listen: {e}"))?;
    let listener = socket.listen(1024)
        .map_err(|e| format!("Listen for hole punch: {e}"))?;

    let listen_addr = listener.local_addr().map_err(|e| e.to_string())?;
    assert_eq!(listen_addr.port(), local_port);

    // Spawn listener acceptor
    let accept_fut = async {
        tokio::select! {
            result = listener.accept() => {
                match result {
                    Ok((stream, _)) => Ok(stream),
                    Err(e) => Err(format!("Accept: {e}")),
                }
            }
        }
    };

    // Spawn outgoing connection attempt from the SAME local port
    let connect_fut = async {
        let out_socket = match bind_reusable_socket(local_port) {
            Ok(s) => s,
            Err(e) => return Err(format!("Bind reusable socket for connect: {e}")),
        };
        tokio::select! {
            result = out_socket.connect(remote_addr) => {
                match result {
                    Ok(stream) => Ok(stream),
                    Err(e) => Err(format!("Connect: {e}")),
                }
            }
        }
    };

    // Race both: whoever wins first is the established connection
    let result = tokio::select! {
        r = accept_fut => r,
        r = connect_fut => r,
    };

    timeout(Duration::from_millis(timeout_ms), async { result })
        .await
        .map_err(|_| format!("Hole punch timed out after {timeout_ms}ms"))?
}

/// NAT classification based on STUN behavior
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NatType {
    Open,
    FullCone,
    RestrictedCone,
    PortRestrictedCone,
    Symmetric,
}

/// Classify the NAT type using STUN server interaction.
/// This is a placeholder that returns the most common home NAT type.
pub fn classify_nat_type() -> NatType {
    // In a full implementation, this would make multiple STUN requests
    // with different source addresses to determine the NAT behavior.
    NatType::PortRestrictedCone
}

/// Check if a peer is reachable via direct TCP connection.
pub async fn is_reachable(addr: SocketAddr, timeout_ms: u64) -> bool {
    timeout(Duration::from_millis(timeout_ms), TcpStream::connect(addr))
        .await
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_nat_classification() {
        let nat = classify_nat_type();
        assert_eq!(nat, NatType::PortRestrictedCone);
    }

    #[tokio::test]
    async fn test_unreachable_host() {
        let result = is_reachable("127.0.0.1:1".parse().unwrap(), 100).await;
        assert!(!result);
    }
}
