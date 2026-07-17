use std::fs;

fn main() {
    let mut s = fs::read_to_string("desktop/p2ptransfer-cli/src/main.rs").unwrap();
    
    let old_tc = "                                let std_file = f.try_clone().await.expect(\"Failed to clone\");";
    let new_tc = "                                let std_file = f.try_clone().await.expect(\"Failed to clone\").into_std().await;";
    
    if s.contains(old_tc) { s = s.replace(old_tc, new_tc); }

    fs::write("desktop/p2ptransfer-cli/src/main.rs", s).unwrap();
}