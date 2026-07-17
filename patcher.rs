use std::fs;

fn main() {
    let mut s = fs::read_to_string("desktop/p2ptransfer-cli/src/main.rs").unwrap();

    // 1. Remove stun/relay/nat imports
    s = s.replace("use p2ptransfer_core::network::{nat, relay, stun};", "");

    // 2. Add connections to Send args
    s = s.replace(
        "        /// Chunk size in bytes (default 16MB)\r\n        #[arg(long, default_value_t = 16 * 1024 * 1024)]\r\n        chunk_size: usize,",
        "        /// Chunk size in bytes (default 16MB)\n        #[arg(long, default_value_t = 16 * 1024 * 1024)]\n        chunk_size: usize,\n        /// Number of parallel connections\n        #[arg(long)]\n        connections: Option<usize>,"
    );
    if !s.contains("connections: Option<usize>") {
        s = s.replace(
            "        chunk_size: usize,\n        /// Relay",
            "        chunk_size: usize,\n        /// Number of parallel connections\n        #[arg(long)]\n        connections: Option<usize>,\n        /// Relay"
        );
        if !s.contains("connections: Option<usize>") {
            s = s.replace(
                "        chunk_size: usize,\r\n        /// Relay",
                "        chunk_size: usize,\n        /// Number of parallel connections\n        #[arg(long)]\n        connections: Option<usize>,\n        /// Relay"
            );
        }
    }

    // 3. Add connections to P2pConfig
    s = s.replace(
        "fn default_data_dir() -> PathBuf {",
        "fn default_connections() -> usize { 4 }\n\nfn default_data_dir() -> PathBuf {"
    );
    s = s.replace(
        "    #[serde(default)]\r\n    relay_server: Option<String>,\r\n}",
        "    #[serde(default)]\n    relay_server: Option<String>,\n    #[serde(default = \"default_connections\")]\n    connections: usize,\n}"
    );
    s = s.replace(
        "    #[serde(default)]\n    relay_server: Option<String>,\n}",
        "    #[serde(default)]\n    relay_server: Option<String>,\n    #[serde(default = \"default_connections\")]\n    connections: usize,\n}"
    );
    s = s.replace(
        "            relay_server: None,\r\n        }\r\n    }\r\n}",
        "            relay_server: None,\n            connections: default_connections(),\n        }\n    }\n}"
    );
    s = s.replace(
        "            relay_server: None,\n        }\n    }\n}",
        "            relay_server: None,\n            connections: default_connections(),\n        }\n    }\n}"
    );

    // 4. Wrap TransferEngine in Arc inside cmd_send
    s = s.replace(
        "let engine = TransferEngine::new(4);",
        "let engine = Arc::new(TransferEngine::new(4));"
    );

    // 5. Add TAG_SESSION_JOIN
    s = s.replace(
        "const TAG_SERVER_HELLO: u8 = 0x06;",
        "const TAG_SERVER_HELLO: u8 = 0x06;\nconst TAG_SESSION_JOIN: u8 = 0x07;"
    );

    // 6. Make send_tagged and receive_tagged generic
    s = s.replace(
        "async fn send_tagged(stream: &mut TcpStream, tag: u8, payload: &[u8]) -> Result<()> {",
        "async fn send_tagged<W: tokio::io::AsyncWrite + Unpin>(stream: &mut W, tag: u8, payload: &[u8]) -> Result<()> {"
    );
    s = s.replace(
        "async fn receive_tagged(stream: &mut TcpStream) -> Result<(u8, Vec<u8>)> {",
        "async fn receive_tagged<R: tokio::io::AsyncRead + Unpin>(stream: &mut R) -> Result<(u8, Vec<u8>)> {"
    );

    fs::write("desktop/p2ptransfer-cli/src/main.rs", s).unwrap();
    println!("Patched successfully");
}