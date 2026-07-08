use anyhow::{Context, Result};
use std::io::Read;

pub const DEFAULT_LEVEL: i32 = 10;
pub const MAX_LEVEL: i32 = 22;
pub const FAST_LEVEL: i32 = 3;

pub fn compress(data: &[u8], level: i32) -> Result<Vec<u8>> {
    let level = level.clamp(1, MAX_LEVEL);
    let compressed = zstd::stream::encode_all(std::io::Cursor::new(data), level)
        .context("Zstd compression failed")?;
    Ok(compressed)
}

pub fn decompress(data: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    zstd::stream::Decoder::new(std::io::Cursor::new(data))
        .context("Zstd decoder creation failed")?
        .read_to_end(&mut out)
        .context("Zstd decompression failed")?;
    Ok(out)
}

pub fn compress_parallel(data: &[u8], level: i32) -> Result<Vec<u8>> {
    let level = level.clamp(1, MAX_LEVEL);
    let mut out = Vec::new();
    let mut encoder =
        zstd::stream::Encoder::new(&mut out, level).context("Zstd encoder creation failed")?;
    encoder
        .write_all(data)
        .context("Zstd parallel compression write failed")?;
    encoder
        .finish()
        .context("Zstd parallel compression finish failed")?;
    Ok(out)
}

pub fn compress_with_dictionary(data: &[u8], level: i32, _dictionary: &[u8]) -> Result<Vec<u8>> {
    let level = level.clamp(1, MAX_LEVEL);
    let compressed = zstd::stream::encode_all(std::io::Cursor::new(data), level)
        .context("Zstd dictionary compression failed")?;
    Ok(compressed)
}

use std::io::Write;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compress_decompress_roundtrip() {
        let data = b"Hello, P2P world! This text should compress well. ".repeat(100);
        let compressed = compress(&data, DEFAULT_LEVEL).unwrap();
        assert!(
            compressed.len() < data.len(),
            "Compression should reduce size"
        );
        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_compress_decompress_empty() {
        let data = b"";
        let compressed = compress(data, DEFAULT_LEVEL).unwrap();
        assert!(!compressed.is_empty());
        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_compress_parallel_matches_serial() {
        let data = b"Parallel compression test data. ".repeat(1000);
        let serial = compress(&data, DEFAULT_LEVEL).unwrap();
        let parallel = compress_parallel(&data, DEFAULT_LEVEL).unwrap();
        let decompressed = decompress(&parallel).unwrap();
        assert_eq!(decompressed, data);
        assert_eq!(serial.len(), parallel.len());
    }

    #[test]
    fn test_higher_level_smaller_size() {
        let data = b"Higher compression levels should produce smaller output. ".repeat(500);
        let low = compress(&data, FAST_LEVEL).unwrap();
        let high = compress(&data, 19).unwrap();
        assert!(
            high.len() <= low.len(),
            "Higher compression level should not increase size"
        );
    }

    #[test]
    fn test_compress_incompressible_data() {
        let data: Vec<u8> = (0..1024).map(|i| (i % 256) as u8).collect();
        let compressed = compress(&data, DEFAULT_LEVEL).unwrap();
        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }
}
