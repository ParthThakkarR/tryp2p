package com.p2p.core.model;

import java.nio.file.Path;
import java.util.Objects;
import java.util.Set;

/**
 * Metadata describing a file or directory for transfer.
 * Immutable and thread-safe.
 */
public final class FileMetadata {

    public static final Set<String> INCOMPRESSIBLE_EXTENSIONS = Set.of(
            "zip", "gz", "bz2", "xz", "7z", "rar", "zst", "lz4",
            "jpg", "jpeg", "png", "gif", "webp", "avif", "heic",
            "mp4", "mkv", "avi", "mov", "webm", "flv",
            "mp3", "aac", "ogg", "flac", "opus", "wma",
            "pdf", "docx", "xlsx", "pptx"
    );

    private final String fileName;
    private final String relativePath;
    private final long fileSize;
    private final String sha256Hash;
    private final long lastModified;
    private final boolean directory;
    private final boolean compressible;
    private final long totalChunks;
    private final long chunkSize;

    private FileMetadata(Builder builder) {
        this.fileName = Objects.requireNonNull(builder.fileName, "fileName must not be null");
        this.relativePath = Objects.requireNonNull(builder.relativePath, "relativePath must not be null");
        this.fileSize = builder.fileSize;
        this.sha256Hash = builder.sha256Hash;
        this.lastModified = builder.lastModified;
        this.directory = builder.directory;
        this.compressible = builder.compressible;
        this.chunkSize = builder.chunkSize;
        this.totalChunks = builder.directory ? 0 : calculateTotalChunks(fileSize, chunkSize);

        if (!directory && fileSize < 0) {
            throw new IllegalArgumentException("File size must be non-negative, got: " + fileSize);
        }
        if (!directory && chunkSize <= 0) {
            throw new IllegalArgumentException("Chunk size must be positive, got: " + chunkSize);
        }
    }

    private static long calculateTotalChunks(long fileSize, long chunkSize) {
        if (fileSize <= 0 || chunkSize <= 0) return 0;
        return (fileSize + chunkSize - 1) / chunkSize;
    }

    /**
     * Determines whether a file is compressible based on its extension.
     *
     * @param path the file path to check
     * @return true if the file extension is not in the known incompressible set
     */
    public static boolean isCompressible(Path path) {
        String name = path.getFileName().toString().toLowerCase();
        int dotIndex = name.lastIndexOf('.');
        if (dotIndex < 0) return true;
        String ext = name.substring(dotIndex + 1);
        return !INCOMPRESSIBLE_EXTENSIONS.contains(ext);
    }

    // --- Getters ---

    public String getFileName() { return fileName; }
    public String getRelativePath() { return relativePath; }
    public long getFileSize() { return fileSize; }
    public String getSha256Hash() { return sha256Hash; }
    public long getLastModified() { return lastModified; }
    public boolean isDirectory() { return directory; }
    public boolean isCompressible() { return compressible; }
    public long getTotalChunks() { return totalChunks; }
    public long getChunkSize() { return chunkSize; }

    /**
     * Returns a human-readable file size string (e.g. "1.5 MB").
     */
    public String getFormattedSize() {
        if (fileSize < 1024) return fileSize + " B";
        if (fileSize < 1024 * 1024) return String.format("%.1f KB", fileSize / 1024.0);
        if (fileSize < 1024 * 1024 * 1024) return String.format("%.1f MB", fileSize / (1024.0 * 1024));
        return String.format("%.2f GB", fileSize / (1024.0 * 1024 * 1024));
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof FileMetadata other)) return false;
        return fileSize == other.fileSize
                && Objects.equals(relativePath, other.relativePath)
                && Objects.equals(sha256Hash, other.sha256Hash);
    }

    @Override
    public int hashCode() {
        return Objects.hash(relativePath, fileSize, sha256Hash);
    }

    @Override
    public String toString() {
        return String.format("FileMetadata[name=%s, size=%s, chunks=%d, compressible=%s]",
                fileName, getFormattedSize(), totalChunks, compressible);
    }

    public static Builder builder() {
        return new Builder();
    }

    public static final class Builder {
        private String fileName;
        private String relativePath;
        private long fileSize;
        private String sha256Hash;
        private long lastModified;
        private boolean directory;
        private boolean compressible = true;
        private long chunkSize = 1024 * 1024;

        public Builder fileName(String fileName) { this.fileName = fileName; return this; }
        public Builder relativePath(String relativePath) { this.relativePath = relativePath; return this; }
        public Builder fileSize(long fileSize) { this.fileSize = fileSize; return this; }
        public Builder sha256Hash(String sha256Hash) { this.sha256Hash = sha256Hash; return this; }
        public Builder lastModified(long lastModified) { this.lastModified = lastModified; return this; }
        public Builder directory(boolean directory) { this.directory = directory; return this; }
        public Builder compressible(boolean compressible) { this.compressible = compressible; return this; }
        public Builder chunkSize(long chunkSize) { this.chunkSize = chunkSize; return this; }

        public FileMetadata build() {
            return new FileMetadata(this);
        }
    }
}
