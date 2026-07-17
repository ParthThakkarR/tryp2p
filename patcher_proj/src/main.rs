use std::fs;
use regex::Regex;

fn main() {
    let mut s = fs::read_to_string("../desktop/p2ptransfer-cli/src/main.rs").unwrap();
    let re_stream = Regex::new(r"(?s)    let mut stream = if let Some\(peer_addr\) = peer_addr_opt \{.*?\};\r?\n    set_nodelay\(&stream\);").unwrap();
    
    let new_stream = "    let mut stream = if let Some(peer_addr) = peer_addr_opt {
        match try_connect_fallback(peer_addr).await {
            Ok(s) => s,
            Err(e) => anyhow::bail!(\"{e}\"),
        }
    } else {
        anyhow::bail!(\"Cannot resolve '{peer}'. Use IP:port format.\");
    };
    set_nodelay(&stream);";
    s = re_stream.replace(&s, new_stream).into_owned();
    fs::write("../desktop/p2ptransfer-cli/src/main.rs", s).unwrap();
}