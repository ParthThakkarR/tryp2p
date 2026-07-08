use serde::{Deserialize, Serialize};

pub const MAGIC: &[u8; 4] = b"P2PT";
pub const PROTOCOL_VERSION: u8 = 1;
pub const DEFAULT_DISCOVERY_PORT: u16 = 9876;
pub const MULTICAST_GROUP: &str = "224.0.0.251";
pub const BEACON_INTERVAL_SECS: u64 = 5;
pub const PEER_TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Beacon {
    pub device_name: String,
    pub p2ptransfer_version: String,
    pub tcp_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeInit {
    pub device_name: String,
    pub public_key: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeResponse {
    pub device_name: String,
    pub public_key: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferRequest {
    pub session_id: String,
    pub file_name: String,
    pub file_size: u64,
    pub checksum: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkHeader {
    pub session_id: String,
    pub chunk_index: u64,
    pub chunk_size: u32,
    pub checksum: u64,
}
