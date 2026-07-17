use std::fs;

fn main() {
    let mut s = fs::read_to_string("desktop/p2ptransfer-cli/src/main.rs").unwrap();

    // 1. Fix Commands::Send match
    s = s.replace(
        "            chunk_size,\n            relay,\n        } => cmd_send(path, peer, compression, chunk_size, relay, &config).await,",
        "            chunk_size,\n            relay,\n            connections,\n        } => cmd_send(path, peer, compression, chunk_size, relay, connections, &config).await,"
    );

    // 2. Fix cmd_relay
    if let Some(start) = s.find("async fn cmd_relay(port: u16) -> Result<()> {") {
        if let Some(end) = s[start..].find("\n}\n") {
            let old = &s.clone()[start..start+end+3];
            s = s.replace(old, "async fn cmd_relay(_port: u16) -> Result<()> {\n    anyhow::bail!(\"Relay feature disabled\");\n}\n");
        }
    }

    // 3. Fix cmd_whoami
    if let Some(start) = s.find("async fn cmd_whoami(name: String, config: &P2pConfig) -> Result<()> {") {
        if let Some(end) = s[start..].find("\n}\n") {
            let old = &s.clone()[start..start+end+3];
            s = s.replace(old, "async fn cmd_whoami(_name: String, _config: &P2pConfig) -> Result<()> {\n    anyhow::bail!(\"Whoami feature disabled\");\n}\n");
        }
    }

    // 4. Fix try_connect_fallback
    if let Some(start) = s.find("async fn try_connect_fallback(") {
        if let Some(end) = s[start..].find("\n}\n") {
            let old = &s.clone()[start..start+end+3];
            let new_func = "async fn try_connect_fallback(
    peer_addr: SocketAddr,
    _relay_addr: Option<&SocketAddr>,
    _peer_id_opt: Option<String>,
) -> Result<TcpStream> {
    tcp::connect(peer_addr).await
}
";
            s = s.replace(old, new_func);
        }
    }

    fs::write("desktop/p2ptransfer-cli/src/main.rs", s).unwrap();
    println!("Fixed features");
}