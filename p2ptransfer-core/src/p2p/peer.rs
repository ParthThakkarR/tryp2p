use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub device_name: String,
    pub p2ptransfer_version: String,
    pub tcp_port: u16,
    pub socket_addr: SocketAddr,
    pub last_seen_epoch: u64,
}

impl PeerInfo {
    pub fn new(device_name: String, tcp_port: u16, socket_addr: SocketAddr) -> Self {
        Self {
            device_name,
            p2ptransfer_version: env!("CARGO_PKG_VERSION").to_string(),
            tcp_port,
            socket_addr,
            last_seen_epoch: now_epoch(),
        }
    }

    pub fn touch(&mut self) {
        self.last_seen_epoch = now_epoch();
    }

    pub fn is_stale(&self, timeout: Duration) -> bool {
        now_epoch().saturating_sub(self.last_seen_epoch) > timeout.as_secs()
    }
}

fn now_epoch() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
