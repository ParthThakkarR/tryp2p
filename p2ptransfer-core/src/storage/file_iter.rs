use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub struct FileIterator {
    entries: Vec<PathBuf>,
    index: usize,
}

impl FileIterator {
    pub fn new(path: &Path) -> Result<Self> {
        if path.is_file() {
            return Ok(Self {
                entries: vec![path.to_path_buf()],
                index: 0,
            });
        }

        let mut entries = Vec::new();
        collect_files(path, &mut entries).context("Failed to collect files")?;
        entries.sort();

        Ok(Self { entries, index: 0 })
    }

    pub fn total_files(&self) -> usize {
        self.entries.len()
    }

    pub fn remaining(&self) -> usize {
        self.entries.len().saturating_sub(self.index)
    }
}

impl Iterator for FileIterator {
    type Item = PathBuf;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.entries.len() {
            return None;
        }
        let entry = self.entries[self.index].clone();
        self.index += 1;
        Some(entry)
    }
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if !dir.is_dir() {
        out.push(dir.to_path_buf());
        return Ok(());
    }

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}

pub fn read_file_chunk(path: &Path, offset: u64, length: usize) -> Result<Vec<u8>> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("Failed to open file: {}", path.display()))?;

    let mmap = unsafe {
        memmap2::Mmap::map(&file)
            .with_context(|| format!("Failed to mmap file: {}", path.display()))?
    };

    let start = offset as usize;
    let end = (start + length).min(mmap.len());
    Ok(mmap[start..end].to_vec())
}

pub fn file_size(path: &Path) -> Result<u64> {
    Ok(std::fs::metadata(path)
        .with_context(|| format!("Failed to read metadata for: {}", path.display()))?
        .len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_iterator_single_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");
        std::fs::write(&path, b"hello").unwrap();

        let iter = FileIterator::new(&path).unwrap();
        assert_eq!(iter.total_files(), 1);
    }

    #[test]
    fn test_file_iterator_directory() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), b"a").unwrap();
        std::fs::write(dir.path().join("b.txt"), b"b").unwrap();
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("c.txt"), b"c").unwrap();

        let iter = FileIterator::new(dir.path()).unwrap();
        assert_eq!(iter.total_files(), 3);

        let files: Vec<_> = iter.collect();
        assert_eq!(files.len(), 3);
    }

    #[test]
    fn test_file_iterator_empty_directory() {
        let dir = tempfile::tempdir().unwrap();
        let iter = FileIterator::new(dir.path()).unwrap();
        assert_eq!(iter.total_files(), 0);
    }

    #[test]
    fn test_read_file_chunk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.bin");
        let data: Vec<u8> = (0..100).collect();
        std::fs::write(&path, &data).unwrap();

        let chunk = read_file_chunk(&path, 10, 20).unwrap();
        assert_eq!(chunk.len(), 20);
        assert_eq!(chunk[0], 10);
        assert_eq!(chunk[19], 29);
    }

    #[test]
    fn test_read_file_chunk_bounds() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("small.bin");
        let data: Vec<u8> = (0..10).collect();
        std::fs::write(&path, &data).unwrap();

        let chunk = read_file_chunk(&path, 5, 20).unwrap();
        assert_eq!(chunk.len(), 5);
        assert_eq!(chunk, vec![5, 6, 7, 8, 9]);
    }

    #[test]
    fn test_file_size() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("size_test.bin");
        std::fs::write(&path, vec![0u8; 12345]).unwrap();

        assert_eq!(file_size(&path).unwrap(), 12345);
    }

    #[test]
    fn test_iterator_remaining() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("x.txt"), b"x").unwrap();
        std::fs::write(dir.path().join("y.txt"), b"y").unwrap();

        let mut iter = FileIterator::new(dir.path()).unwrap();
        assert_eq!(iter.remaining(), 2);
        iter.next();
        assert_eq!(iter.remaining(), 1);
        iter.next();
        assert_eq!(iter.remaining(), 0);
    }
}
