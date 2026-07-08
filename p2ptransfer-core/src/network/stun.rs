use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;

const STUN_BINDING_REQUEST: [u8; 20] = [
    0x00, 0x01, 0x00, 0x00, // Binding Request type=0x0001, length=0
    0x21, 0x12, 0xa4, 0x42, // Magic cookie
    0x00, 0x00, 0x00, 0x00, // Transaction ID (12 bytes)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

const DEFAULT_STUN_SERVER: &str = "stun.l.google.com:19302";

#[derive(Debug, Clone)]
pub struct StunResult {
    pub public_addr: SocketAddr,
    pub rtt: Duration,
}

/// Perform a STUN binding request to discover the public IP and port.
pub fn discover_address(stun_server: Option<&str>) -> Result<StunResult, String> {
    let server = stun_server.unwrap_or(DEFAULT_STUN_SERVER);
    let sock = UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("Bind: {e}"))?;
    sock.set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| format!("Set timeout: {e}"))?;
    sock.connect(server).map_err(|e| format!("Connect: {e}"))?;

    let start = std::time::Instant::now();
    sock.send(&STUN_BINDING_REQUEST)
        .map_err(|e| format!("Send: {e}"))?;

    let mut buf = [0u8; 512];
    let n = sock.recv(&mut buf).map_err(|e| format!("Recv: {e}"))?;
    let rtt = start.elapsed();

    if n < 20 || buf[0..2] != [0x01, 0x01] {
        return Err("Not a STUN Binding Response".into());
    }

    let mut offset = 20;
    while offset + 4 < n {
        let attr_type = u16::from_be_bytes([buf[offset], buf[offset + 1]]);
        let attr_len = u16::from_be_bytes([buf[offset + 2], buf[offset + 3]]) as usize;
        offset += 4;

        if attr_type == 0x0020 {
            // XOR-MAPPED-ADDRESS
            if offset + attr_len <= n && attr_len >= 8 {
                let family = buf[offset + 1];
                let xport = u16::from_be_bytes([buf[offset + 2], buf[offset + 3]]);
                let port = xport ^ 0x2112;
                if family == 0x01 {
                    // IPv4
                    let ip = [
                        buf[offset + 4] ^ 0x21,
                        buf[offset + 5] ^ 0x12,
                        buf[offset + 6] ^ 0xa4,
                        buf[offset + 7] ^ 0x42,
                    ];
                    let addr: SocketAddr =
                        format!("{}.{}.{}.{}:{}", ip[0], ip[1], ip[2], ip[3], port)
                            .parse()
                            .map_err(|_| "Invalid STUN address")?;
                    return Ok(StunResult { public_addr: addr, rtt });
                }
            }
        }
        offset += attr_len;
    }

    Err("No XOR-MAPPED-ADDRESS in STUN response".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stun_discovery() {
        let result = discover_address(None);
        assert!(result.is_ok(), "STUN failed: {:?}", result.err());
        let r = result.unwrap();
        eprintln!("Public: {}, RTT: {:?}", r.public_addr, r.rtt);
        assert!(r.rtt < Duration::from_secs(10));
    }
}
