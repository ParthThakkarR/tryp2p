package com.p2p.transfer.engine;

import com.p2p.core.model.FileMetadata;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.ArrayDeque;
import java.util.Deque;
import java.util.Objects;

/**
 * Adaptively determines the optimal chunk size for file transfers based on
 * measured round-trip times and bandwidth-delay product. The chunk size is
 * clamped between 512 KB and 2 MB. Thread-safe for concurrent recording of
 * RTT samples.
 */
public final class AdaptiveChunkSizer {

    // --- Constants ---

    private static final Logger log = LoggerFactory.getLogger(AdaptiveChunkSizer.class);

    private static final long MIN_CHUNK_SIZE = 512 * 1024;          // 512 KB
    private static final long MAX_CHUNK_SIZE = 2 * 1024 * 1024;     // 2 MB
    private static final int RTT_SAMPLES = 10;
    private static final double HIGH_RTT_THRESHOLD_MS = 100.0;
    private static final double LOW_RTT_THRESHOLD_MS = 10.0;

    // --- Fields ---

    private final Deque<Long> rttSamples = new ArrayDeque<>(RTT_SAMPLES);
    private long currentChunkSize;
    private double estimatedThroughputBps = 0;

    // --- Constructor ---

    /**
     * Creates an AdaptiveChunkSizer with the given initial chunk size,
     * clamped to the allowable range.
     *
     * @param initialChunkSize the starting chunk size in bytes
     */
    public AdaptiveChunkSizer(long initialChunkSize) {
        this.currentChunkSize = Math.max(MIN_CHUNK_SIZE, Math.min(MAX_CHUNK_SIZE, initialChunkSize));
    }

    // --- Public API ---

    /**
     * Returns the current adaptive chunk size.
     *
     * @return chunk size in bytes
     */
    public long getCurrentChunkSize() {
        return currentChunkSize;
    }

    /**
     * Records a round-trip time sample and triggers a chunk size adjustment.
     *
     * @param rttMs the measured RTT in milliseconds; must be non-negative
     * @throws IllegalArgumentException if rttMs is negative
     */
    public void recordRtt(long rttMs) {
        if (rttMs < 0) {
            throw new IllegalArgumentException("rttMs must be non-negative: " + rttMs);
        }
        rttSamples.addLast(rttMs);
        if (rttSamples.size() > RTT_SAMPLES) {
            rttSamples.removeFirst();
        }
        adjustChunkSize();
    }

    /**
     * Records a throughput measurement used for bandwidth-delay product
     * calculations.
     *
     * @param bytesTransferred number of bytes transferred in this sample
     * @param durationMs       time taken in milliseconds
     */
    public void recordThroughput(long bytesTransferred, long durationMs) {
        if (durationMs > 0) {
            estimatedThroughputBps = (double) bytesTransferred / durationMs * 1000.0;
        }
    }

    /**
     * Computes the optimal chunk size based on bandwidth and RTT using the
     * bandwidth-delay product.
     *
     * @param bandwidthBps estimated bandwidth in bits per second
     * @param rttMs        estimated round-trip time in milliseconds
     * @return optimal chunk size in bytes, clamped to [MIN_CHUNK_SIZE, MAX_CHUNK_SIZE]
     */
    public long getOptimalChunkSize(long bandwidthBps, long rttMs) {
        if (bandwidthBps < 0) {
            throw new IllegalArgumentException("bandwidthBps must be non-negative: " + bandwidthBps);
        }
        if (rttMs < 0) {
            throw new IllegalArgumentException("rttMs must be non-negative: " + rttMs);
        }
        double bdp = (double) bandwidthBps * rttMs / 1000.0;
        long optimal = Math.max(MIN_CHUNK_SIZE, Math.min(MAX_CHUNK_SIZE, (long) (bdp * 2)));
        return optimal;
    }

    // --- Internal ---

    private void adjustChunkSize() {
        if (rttSamples.isEmpty()) {
            return;
        }
        double avgRtt = rttSamples.stream().mapToLong(Long::longValue).average().orElse(0);

        long oldSize = currentChunkSize;
        if (avgRtt > HIGH_RTT_THRESHOLD_MS) {
            currentChunkSize = Math.max(MIN_CHUNK_SIZE, currentChunkSize / 2);
        } else if (avgRtt < LOW_RTT_THRESHOLD_MS) {
            currentChunkSize = Math.min(MAX_CHUNK_SIZE, currentChunkSize * 2);
        }

        if (oldSize != currentChunkSize) {
            log.debug("Chunk size adjusted: {} -> {} (avg RTT: {:.1f}ms)", oldSize, currentChunkSize, avgRtt);
        }
    }

    // --- Metadata ---

    /**
     * Creates a new FileMetadata with the current adaptive chunk size applied,
     * preserving all other fields from the input metadata.
     *
     * @param metadata the source metadata
     * @return new metadata with updated chunk size
     * @throws NullPointerException if metadata is null
     */
    public FileMetadata withAdaptiveChunkSize(FileMetadata metadata) {
        Objects.requireNonNull(metadata, "metadata must not be null");
        return FileMetadata.builder()
                .fileName(metadata.getFileName())
                .relativePath(metadata.getRelativePath())
                .fileSize(metadata.getFileSize())
                .sha256Hash(metadata.getSha256Hash())
                .lastModified(metadata.getLastModified())
                .directory(metadata.isDirectory())
                .compressible(metadata.isCompressible())
                .chunkSize(currentChunkSize)
                .build();
    }

    // --- Object ---

    @Override
    public String toString() {
        return String.format("AdaptiveChunkSizer[currentSize=%d, throughput=%.0f bps, rttSamples=%d]",
                currentChunkSize, estimatedThroughputBps, rttSamples.size());
    }
}
