use std::fs;

fn main() {
    let mut s = fs::read_to_string("desktop/p2ptransfer-cli/src/main.rs").unwrap();

    // 1. Fix _relay
    s = s.replace("    _relay: Option<String>,\n    connections: Option<usize>,", "    relay: Option<String>,\n    connections: Option<usize>,");
    s = s.replace("    _relay: Option<String>,\r\n    connections: Option<usize>,", "    relay: Option<String>,\r\n    connections: Option<usize>,");

    // 2. Fix imports
    s = s.replace("use tokio::io::AsyncReadExt;", "use tokio::io::{AsyncReadExt, AsyncWriteExt, AsyncSeekExt};");

    // 3. Fix try_clone
    s = s.replace(
        "let mut std_file = f.try_clone().expect(\"Failed to clone file descriptor\");",
        "let mut std_file = f.try_clone().await.expect(\"Failed to clone file descriptor\");"
    );

    fs::write("desktop/p2ptransfer-cli/src/main.rs", s).unwrap();
}