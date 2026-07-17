use std::fs;

fn main() {
    let mut s = fs::read_to_string("desktop/p2ptransfer-cli/src/main.rs").unwrap();
    let old = "    let relay_addr = relay\r\n        .as_deref()\r\n        .or(config.relay_server.as_deref())\r\n        .map(|r| r.parse::<SocketAddr>().expect(\"Invalid relay address\"));";
    let old_lf = old.replace(\"\r\n\", \"\n\");
    
    if s.contains(old) { s = s.replace(old, \"\"); }
    else if s.contains(&old_lf) { s = s.replace(&old_lf, \"\"); }
    
    fs::write("desktop/p2ptransfer-cli/src/main.rs", s).unwrap();
}