use std::cmp;

pub const DEFAULT_CHUNK_SIZE: usize = 16 * 1024 * 1024; // 16 MB
pub const MIN_CHUNK_SIZE: usize = 512 * 1024; // 512 KB
pub const MAX_CHUNK_SIZE: usize = 64 * 1024 * 1024; // 64 MB

#[derive(Debug, Clone)]
pub struct Chunker {
    chunk_size: usize,
    file_size: u64,
}

impl Chunker {
    pub fn new(chunk_size: usize, file_size: u64) -> Self {
        let chunk_size = chunk_size.clamp(MIN_CHUNK_SIZE, MAX_CHUNK_SIZE);
        Self {
            chunk_size,
            file_size,
        }
    }

    pub fn chunk_size(&self) -> usize {
        self.chunk_size
    }

    pub fn file_size(&self) -> u64 {
        self.file_size
    }

    pub fn total_chunks(&self) -> u64 {
        if self.file_size == 0 {
            return 1;
        }
        self.file_size.div_ceil(self.chunk_size as u64)
    }

    pub fn chunk_offset(&self, index: u64) -> u64 {
        index * self.chunk_size as u64
    }

    pub fn chunk_length(&self, index: u64) -> usize {
        let offset = self.chunk_offset(index);
        let remaining = self.file_size.saturating_sub(offset);
        cmp::min(remaining, self.chunk_size as u64) as usize
    }

    pub fn adaptive_chunk_size(rtt_ms: f64) -> usize {
        if rtt_ms > 100.0 {
            MAX_CHUNK_SIZE
        } else if rtt_ms > 20.0 {
            8 * 1024 * 1024
        } else {
            DEFAULT_CHUNK_SIZE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunker_zero_file() {
        let c = Chunker::new(DEFAULT_CHUNK_SIZE, 0);
        assert_eq!(c.total_chunks(), 1);
        assert_eq!(c.chunk_length(0), 0);
    }

    #[test]
    fn test_chunker_exact_one_chunk() {
        let cs = MIN_CHUNK_SIZE as u64;
        let c = Chunker::new(MIN_CHUNK_SIZE, cs);
        assert_eq!(c.total_chunks(), 1);
        assert_eq!(c.chunk_length(0), cs as usize);
        assert_eq!(c.chunk_offset(0), 0);
    }

    #[test]
    fn test_chunker_multiple_chunks() {
        let cs = 1024 * 1024;
        let c = Chunker::new(cs, 2500 * 1024);
        assert_eq!(c.total_chunks(), 3);
        assert_eq!(c.chunk_length(0), cs);
        assert_eq!(c.chunk_length(1), cs);
        assert_eq!(c.chunk_length(2), 452 * 1024);
        assert_eq!(c.chunk_offset(0), 0);
        assert_eq!(c.chunk_offset(1), cs as u64);
        assert_eq!(c.chunk_offset(2), 2 * cs as u64);
    }

    #[test]
    fn test_chunker_clamps_min_size() {
        let c = Chunker::new(128, 1000);
        assert_eq!(c.chunk_size(), MIN_CHUNK_SIZE);
    }

    #[test]
    fn test_chunker_clamps_max_size() {
        let c = Chunker::new(128 * 1024 * 1024, 1000);
        assert_eq!(c.chunk_size(), MAX_CHUNK_SIZE);
    }

    #[test]
    fn test_adaptive_chunk_size() {
        let high_rtt = Chunker::adaptive_chunk_size(200.0);
        assert_eq!(high_rtt, MAX_CHUNK_SIZE);

        let medium_rtt = Chunker::adaptive_chunk_size(50.0);
        assert_eq!(medium_rtt, 8 * 1024 * 1024);

        let low_rtt = Chunker::adaptive_chunk_size(5.0);
        assert_eq!(low_rtt, DEFAULT_CHUNK_SIZE);
    }
}
