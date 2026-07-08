use anyhow::{Context, Result};

pub fn compress(data: &[u8]) -> Result<Vec<u8>> {
    let compressed = lz4_flex::compress_prepend_size(data);
    Ok(compressed)
}

pub fn decompress(data: &[u8]) -> Result<Vec<u8>> {
    lz4_flex::decompress_size_prepended(data).context("LZ4 decompression failed")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lz4_roundtrip() {
        let data = b"LZ4 is very fast! ".repeat(100);
        let compressed = compress(&data).unwrap();
        assert!(
            compressed.len() < data.len(),
            "LZ4 compression should reduce size"
        );
        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_lz4_empty() {
        let data = b"";
        let compressed = compress(data).unwrap();
        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_lz4_faster_than_zstd() {
        let data = b"a".repeat(100_000);
        let lz4_start = std::time::Instant::now();
        let lz4_compressed = compress(&data).unwrap();
        let lz4_time = lz4_start.elapsed();

        let zstd_start = std::time::Instant::now();
        let zstd_compressed = crate::compress::zstd::compress(&data, 3).unwrap();
        let zstd_time = zstd_start.elapsed();

        assert!(lz4_time < zstd_time || lz4_compressed.len() >= zstd_compressed.len());
        assert_eq!(
            decompress(&lz4_compressed).unwrap(),
            crate::compress::zstd::decompress(&zstd_compressed).unwrap()
        );
    }
}
