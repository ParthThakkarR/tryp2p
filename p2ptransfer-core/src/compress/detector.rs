const ALREADY_COMPRESSED_EXTENSIONS: &[&str] = &[
    "zip", "gz", "gzip", "bz2", "xz", "zst", "zstd", "lz4", "lzma", "rar", "7z", "tgz", "tar",
    "zlib", "mp3", "mp4", "m4a", "m4v", "mkv", "avi", "mov", "wmv", "flv", "webm", "jpg", "jpeg",
    "png", "gif", "webp", "bmp", "tiff", "ico", "pdf", "docx", "xlsx", "pptx", "woff", "woff2",
    "ttf", "otf",
];

const COMPRESSED_MAGIC_BYTES: &[(&[u8], &str)] = &[
    (b"\x1f\x8b", "gzip"),
    (b"\x1f\x9d", "zlib (compress)"),
    (b"\x42\x5a\x68", "bzip2"),
    (b"\x50\x4b\x03\x04", "zip/docx/xlsx/pptx"),
    (b"\x52\x61\x72\x21\x1a\x07", "rar"),
    (b"\x28\xb5\x2f\xfd", "zstd"),
    (b"\x89\x50\x4e\x47\x0d\x0a\x1a\x0a", "png"),
    (b"\xff\xd8\xff", "jpeg"),
    (b"\x00\x00\x00\x18\x66\x74\x79\x70", "mp4"),
    (b"\x1a\x45\xdf\xa3", "webm/mkv"),
    (b"\x47\x49\x46\x38", "gif"),
    (b"\x52\x49\x46\x46", "webp"),
    (b"\x25\x50\x44\x46", "pdf"),
];

pub fn is_likely_compressed(path: &std::path::Path, first_bytes: &[u8]) -> bool {
    if let Some(ext) = path.extension() {
        let ext_str = ext.to_string_lossy().to_lowercase();
        if ALREADY_COMPRESSED_EXTENSIONS.contains(&ext_str.as_str()) {
            return true;
        }
    }

    for (magic, _name) in COMPRESSED_MAGIC_BYTES {
        if first_bytes.len() >= magic.len() && first_bytes[..magic.len()] == **magic {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_detects_zip_by_extension() {
        assert!(is_likely_compressed(Path::new("archive.zip"), b"notmagic"));
    }

    #[test]
    fn test_detects_gzip_by_magic() {
        let header = b"\x1f\x8b\x08\x00\x00\x00\x00\x00\x00\xff";
        assert!(is_likely_compressed(Path::new("data.bin"), header));
    }

    #[test]
    fn test_detects_png_by_magic() {
        let header = b"\x89\x50\x4e\x47\x0d\x0a\x1a\x0a";
        assert!(is_likely_compressed(Path::new("image.png"), header));
    }

    #[test]
    fn test_plain_text_not_compressed() {
        let header = b"This is a plain text file with no magic bytes.";
        assert!(!is_likely_compressed(Path::new("readme.txt"), header));
    }

    #[test]
    fn test_empty_file_not_compressed() {
        assert!(!is_likely_compressed(Path::new("empty.bin"), &[]));
    }

    #[test]
    fn test_jpeg_detected() {
        let header = b"\xff\xd8\xff\xe0\x00\x10\x4a\x46\x49\x46";
        assert!(is_likely_compressed(Path::new("photo.jpg"), header));
    }

    #[test]
    fn test_mp4_detected() {
        let header = b"\x00\x00\x00\x18\x66\x74\x79\x70\x69\x73\x6f\x6d";
        assert!(is_likely_compressed(Path::new("video.mp4"), header));
    }
}
