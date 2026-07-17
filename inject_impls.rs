use std::fs;

fn main() {
    let mut main_rs = fs::read_to_string("desktop/p2ptransfer-cli/src/main.rs").unwrap();
    let new_send = fs::read_to_string("C:/Users/parth/.gemini/antigravity-ide/brain/bf1e8cbf-8463-4726-b5ac-edcc8c82b3eb/scratch/new_send.rs").unwrap();
    let new_impl = fs::read_to_string("C:/Users/parth/.gemini/antigravity-ide/brain/bf1e8cbf-8463-4726-b5ac-edcc8c82b3eb/scratch/new_impl.rs").unwrap();

    // 1. Replace ReceiverHandler
    let rh_start = main_rs.find("impl ReceiverHandler {").unwrap();
    let rh_end = main_rs[rh_start..].find("\n}\n").unwrap() + rh_start + 3;
    let old_rh = &main_rs.clone()[rh_start..rh_end];
    main_rs = main_rs.replace(old_rh, &new_impl);

    // 2. Replace cmd_send chunk loop
    let send_start = main_rs.find("    // --- Signal handling for graceful pause ---").unwrap();
    let send_end = main_rs[send_start..].find("\n    Ok(())\n}").unwrap() + send_start;
    let old_send = &main_rs.clone()[send_start..send_end];

    let mut send_code = String::new();
    send_code.push_str("    let mut current_start_chunk = start_chunk;\n");
    send_code.push_str("    let bytes_sent_atomic = Arc::new(std::sync::atomic::AtomicI64::new(bytes_sent));\n");
    send_code.push_str("    let connections = connections.unwrap_or(config.connections).max(1);\n");
    send_code.push_str("    let peer_addr = peer_addr_opt.unwrap_or_else(|| \"127.0.0.1:0\".parse().unwrap());\n");
    send_code.push_str(&new_send);

    main_rs = main_rs.replace(old_send, &send_code);

    fs::write("desktop/p2ptransfer-cli/src/main.rs", main_rs).unwrap();
    println!("Injected successfully");
}