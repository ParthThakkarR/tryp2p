use std::fs;

fn main() {
    let mut s = fs::read_to_string("desktop/p2ptransfer-cli/src/main.rs").unwrap();

    // 1. Fix try_clone
    let old_tc = "                                let hash = tokio::task::spawn_blocking(move || {\n                                    let mut std_file = f.try_clone().await.expect(\"Failed to clone file descriptor\");";
    let old_tc2 = "                                let hash = tokio::task::spawn_blocking(move || {\r\n                                    let mut std_file = f.try_clone().await.expect(\"Failed to clone file descriptor\");";
    
    let new_tc = "                                let std_file = f.try_clone().await.expect(\"Failed to clone\");\n                                let hash = tokio::task::spawn_blocking(move || {\n                                    let mut std_file = std_file;";
    
    if s.contains(old_tc) { s = s.replace(old_tc, new_tc); }
    else if s.contains(old_tc2) { s = s.replace(old_tc2, new_tc); }

    // 2. Add AsyncWriteExt and AsyncSeekExt
    s = s.replace("use tokio::io::AsyncReadExt;", "use tokio::io::{AsyncReadExt, AsyncWriteExt, AsyncSeekExt};");
    // if not found, just put it after use tokio::net::TcpListener;
    if !s.contains("AsyncWriteExt") {
        s = s.replace("use tokio::net::TcpListener;", "use tokio::net::TcpListener;\nuse tokio::io::{AsyncWriteExt, AsyncSeekExt};");
    }

    fs::write("desktop/p2ptransfer-cli/src/main.rs", s).unwrap();
}