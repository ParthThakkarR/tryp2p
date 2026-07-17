use std::fs;

fn main() {
    let mut s = fs::read_to_string("desktop/p2ptransfer-cli/src/main.rs").unwrap();
    let new_impl = fs::read_to_string("C:/Users/parth/.gemini/antigravity-ide/brain/bf1e8cbf-8463-4726-b5ac-edcc8c82b3eb/scratch/new_impl.rs").unwrap();
    let new_send = fs::read_to_string("C:/Users/parth/.gemini/antigravity-ide/brain/bf1e8cbf-8463-4726-b5ac-edcc8c82b3eb/scratch/new_send.rs").unwrap();

    // 1. Replace ReceiverHandler body
    let impl_start = s.find("impl ReceiverHandler {").unwrap();
    let impl_end = s[impl_start..].find("async fn cmd_list(").unwrap() + impl_start;
    let old_impl = &s.clone()[impl_start..impl_end];
    
    // new_impl.rs doesn't have \n\n, so we append it
    let mut replaced_impl = new_impl.clone();
    replaced_impl.push_str("\n\n");
    s = s.replace(old_impl, &replaced_impl);

    // 2. Replace cmd_send chunk loop
    let send_start = s.find("    // --- Signal handling for graceful pause ---").unwrap();
    let send_end = s[send_start..].find("async fn try_connect_fallback").unwrap() + send_start;
    // We want to stop exactly after Ok(())\n} which is right before sync fn try_connect_fallback
    let old_send = &s.clone()[send_start..send_end];

    let mut send_code = String::new();
    send_code.push_str("    let mut current_start_chunk = start_chunk;\n");
    send_code.push_str("    let bytes_sent_atomic = Arc::new(std::sync::atomic::AtomicI64::new(bytes_sent as i64));\n");
    send_code.push_str("    let connections = connections.unwrap_or(config.connections).max(1);\n");
    send_code.push_str("    let peer_addr = peer_addr_opt.unwrap_or_else(|| \"127.0.0.1:0\".parse().unwrap());\n");
    send_code.push_str(&new_send);
    send_code.push_str("\n\n");

    s = s.replace(old_send, &send_code);

    fs::write("desktop/p2ptransfer-cli/src/main.rs", s).unwrap();
}