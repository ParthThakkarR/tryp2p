package com.p2p.core.model;

import java.util.Objects;

/**
 * Metadata for a single chunk of a file being transferred.
 * Immutable and thread-safe.
 */
public final class ChunkInfo {

    private final long chunkIndex;
    private final long offset;
    private final int length;
    private final String sha256Hash;
    private final boolean compressed;
    private final int compressedLength;

    private ChunkInfo(Builder builder) {
        this.chunkIndex = builder.chunkIndex;
        this.offset = builder.offset;
        this.length = builder.length;
        this.sha256Hash = builder.sha256Hash;
        this.compressed = builder.compressed;
        this.compressedLength = builder.compressedLength;

        if (chunkIndex < 0) {
            throw new IllegalArgumentException("Chunk index must be non-negative, got: " + chunkIndex);
        }
        if (offset < 0) {
            throw new IllegalArgumentException("Offset must be non-negative, got: " + offset);
        }
        if (length <= 0) {
            throw new IllegalArgumentException("Length must be positive, got: " + length);
        }
        if (compressed && compressedLength <= 0) {
            throw new IllegalArgumentException("Compressed length must be positive when compressed, got: " + compressedLength);
        }
    }

    // --- Getters ---

    public long getChunkIndex() { return chunkIndex; }
    public long getOffset() { return offset; }
    public int getLength() { return length; }
    public String getSha256Hash() { return sha256Hash; }
    public boolean isCompressed() { return compressed; }
    public int getCompressedLength() { return compressedLength; }

    /**
     * Returns the effective data length for this chunk, accounting for compression.
     *
     * @return compressedLength if compressed, otherwise length
     */
    public int getEffectiveLength() {
        return compressed ? compressedLength : length;
    }

    /**
     * Returns the compression ratio (compressed / original).
     *
     * @return ratio where {@code < 1.0} means compression saved space; defaults to 1.0 if not compressed or empty
     */
    public double getCompressionRatio() {
        if (!compressed || length == 0) return 1.0;
        return (double) compressedLength / length;
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof ChunkInfo other)) return false;
        return chunkIndex == other.chunkIndex
                && offset == other.offset
                && length == other.length;
    }

    @Override
    public int hashCode() {
        return Objects.hash(chunkIndex, offset, length);
    }

    @Override
    public String toString() {
        String compInfo = compressed
                ? String.format(", compressed=%d (%.1f%%)", compressedLength, getCompressionRatio() * 100)
                : "";
        return String.format("ChunkInfo[index=%d, offset=%d, length=%d%s]",
                chunkIndex, offset, length, compInfo);
    }

    public static Builder builder() {
        return new Builder();
    }

    public static final class Builder {
        private long chunkIndex;
        private long offset;
        private int length;
        private String sha256Hash;
        private boolean compressed;
        private int compressedLength;

        public Builder chunkIndex(long chunkIndex) { this.chunkIndex = chunkIndex; return this; }
        public Builder offset(long offset) { this.offset = offset; return this; }
        public Builder length(int length) { this.length = length; return this; }
        public Builder sha256Hash(String sha256Hash) { this.sha256Hash = sha256Hash; return this; }
        public Builder compressed(boolean compressed) { this.compressed = compressed; return this; }
        public Builder compressedLength(int compressedLength) { this.compressedLength = compressedLength; return this; }

        public ChunkInfo build() {
            return new ChunkInfo(this);
        }
    }
}
