
use zstd::bulk::Compressor;
fn main() {
    let mut c = Compressor::new(3).unwrap();
    let res = c.compress(b"hello").unwrap();
    println!("{}", res.len());
}

