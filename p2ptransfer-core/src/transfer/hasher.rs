use anyhow::{Context, Result};
use std::io::Read;
use xxhash_rust::xxh64::Xxh64;

pub fn xxhash64(data: &[u8]) -> u64 {
    let mut hasher = Xxh64::new(0);
    hasher.update(data);
    hasher.digest()
}

pub fn blake3_hash(data: &[u8]) -> [u8; 32] {
    let hash = blake3::hash(data);
    *hash.as_bytes()
}

pub fn blake3_hash_reader(mut reader: impl Read) -> Result<[u8; 32]> {
    let mut hasher = blake3::Hasher::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = reader
            .read(&mut buf)
            .context("Failed to read data for hashing")?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let hash = hasher.finalize();
    Ok(*hash.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xxhash64_consistency() {
        let data = b"test data";
        let h1 = xxhash64(data);
        let h2 = xxhash64(data);
        assert_eq!(h1, h2, "XXHash64 must be deterministic");
    }

    #[test]
    fn test_xxhash64_different_inputs() {
        let h1 = xxhash64(b"hello");
        let h2 = xxhash64(b"world");
        assert_ne!(h1, h2, "Different inputs must produce different hashes");
    }

    #[test]
    fn test_xxhash64_empty_input() {
        let h = xxhash64(b"");
        assert_ne!(h, 0, "Empty input hash should not be zero for default seed");
    }

    #[test]
    fn test_blake3_consistency() {
        let data = b"blake3 test";
        let h1 = blake3_hash(data);
        let h2 = blake3_hash(data);
        assert_eq!(h1, h2, "BLAKE3 must be deterministic");
    }

    #[test]
    fn test_blake3_reader() {
        let data = b"streaming hash test data";
        let direct = blake3_hash(data);
        let streamed = blake3_hash_reader(std::io::Cursor::new(data)).unwrap();
        assert_eq!(direct, streamed);
    }

    #[test]
    fn test_blake3_length() {
        let hash = blake3_hash(b"test");
        assert_eq!(hash.len(), 32);
    }

    #[test]
    fn test_xxhash64_length_independent() {
        let small = xxhash64(b"a");
        let large = xxhash64(&[0u8; 10000]);
        assert_ne!(small, large, "Different sized inputs must differ");
    }
}
