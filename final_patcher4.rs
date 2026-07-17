use std::fs;

fn main() {
    let mut s = fs::read_to_string("desktop/p2ptransfer-cli/src/main.rs").unwrap();
    let new_impl = fs::read_to_string("C:/Users/parth/.gemini/antigravity-ide/brain/bf1e8cbf-8463-4726-b5ac-edcc8c82b3eb/scratch/new_impl.rs").unwrap();
    let new_send = fs::read_to_string("C:/Users/parth/.gemini/antigravity-ide/brain/bf1e8cbf-8463-4726-b5ac-edcc8c82b3eb/scratch/new_send.rs").unwrap();

    let impl_start = s.find("impl ReceiverHandler {").unwrap();
    let impl_end = s[impl_start..].find("async fn cmd_list(").unwrap() + impl_start;
    let old_impl = &s.clone()[impl_start..impl_end];
    
    // new_impl.rs doesn't have \n\n
    let mut replaced_impl = new_impl.clone();
    replaced_impl.push_str("\n\n");
    s = s.replace(old_impl, &replaced_impl);

    fs::write("desktop/p2ptransfer-cli/src/main.rs", s).unwrap();
}