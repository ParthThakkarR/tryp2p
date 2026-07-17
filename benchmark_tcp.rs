
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Instant;
use std::thread;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("Usage: bench <server|client>");
        return;
    }
    
    if args[1] == "server" {
        let listener = TcpListener::bind("127.0.0.1:9999").unwrap();
        println!("Server listening on 127.0.0.1:9999");
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = vec![0u8; 1024 * 1024];
        let mut total = 0u64;
        let start = Instant::now();
        loop {
            match stream.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => total += n as u64,
                Err(_) => break,
            }
        }
        let elapsed = start.elapsed().as_secs_f64();
        let mib = total as f64 / (1024.0 * 1024.0);
        println!("Server received: {:.2} MiB in {:.2}s ({:.2} MiB/s)", mib, elapsed, mib / elapsed);
    } else if args[1] == "client" {
        let mut stream = TcpStream::connect("127.0.0.1:9999").unwrap();
        let buf = vec![0u8; 1024 * 1024];
        let total = 5u64 * 1024 * 1024 * 1024; // 5 GB
        let mut sent = 0u64;
        let start = Instant::now();
        while sent < total {
            stream.write_all(&buf).unwrap();
            sent += buf.len() as u64;
        }
        let elapsed = start.elapsed().as_secs_f64();
        let mib = sent as f64 / (1024.0 * 1024.0);
        println!("Client sent: {:.2} MiB in {:.2}s ({:.2} MiB/s)", mib, elapsed, mib / elapsed);
    }
}

