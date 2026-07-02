package com.p2p.transfer.engine;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.Objects;
import java.util.concurrent.atomic.AtomicLong;

/**
 * Measures network bandwidth by tracking bytes transferred over time windows.
 * Maintains both a long-term average and a sliding-window estimate with
 * exponential smoothing. Thread-safe via atomic counters.
 */
public final class BandwidthProfiler {

    // --- Constants ---

    private static final Logger log = LoggerFactory.getLogger(BandwidthProfiler.class);

    private static final int WINDOW_SIZE = 20;
    private static final long MIN_SAMPLE_BYTES = 1024;
    private static final long MIN_SAMPLE_DURATION_MS = 50;

    // --- Fields ---

    private final AtomicLong totalBytesTransferred = new AtomicLong(0);
    private final AtomicLong totalDurationMs = new AtomicLong(0);
    private long currentWindowBytes = 0;
    private long currentWindowStartMs = 0;
    private long estimatedBandwidthBps = 0;

    // --- Public API ---

    /**
     * Records a single transfer sample, updating both the running totals and
     * the sliding-window estimate.
     *
     * @param bytes      number of bytes transferred in this sample
     * @param durationMs time taken in milliseconds
     * @throws IllegalArgumentException if bytes is negative or durationMs is negative
     */
    public void recordTransfer(int bytes, long durationMs) {
        if (bytes < 0) {
            throw new IllegalArgumentException("bytes must be non-negative: " + bytes);
        }
        if (durationMs < 0) {
            throw new IllegalArgumentException("durationMs must be non-negative: " + durationMs);
        }
        if (durationMs == 0) {
            return;
        }

        totalBytesTransferred.addAndGet(bytes);
        totalDurationMs.addAndGet(durationMs);
        currentWindowBytes += bytes;

        if (currentWindowStartMs == 0) {
            currentWindowStartMs = System.currentTimeMillis();
        }

        long windowElapsed = System.currentTimeMillis() - currentWindowStartMs;
        if (windowElapsed >= 1000 || currentWindowBytes >= MIN_SAMPLE_BYTES) {
            if (windowElapsed >= MIN_SAMPLE_DURATION_MS && currentWindowBytes > 0) {
                long bps = (long) ((double) currentWindowBytes / windowElapsed * 1000.0);
                updateEstimate(bps);
            }
            currentWindowBytes = 0;
            currentWindowStartMs = System.currentTimeMillis();
        }
    }

    /**
     * Returns the current exponentially-smoothed bandwidth estimate.
     *
     * @return estimated bandwidth in bits per second
     */
    public long getEstimatedBandwidthBps() {
        return estimatedBandwidthBps;
    }

    /**
     * Returns the long-term average bandwidth across all recorded samples.
     *
     * @return average bandwidth in bits per second, or 0 if no samples
     */
    public long getAverageBandwidthBps() {
        long duration = totalDurationMs.get();
        if (duration <= 0) {
            return 0;
        }
        return (long) ((double) totalBytesTransferred.get() / duration * 1000.0);
    }

    /**
     * Returns a recommended parallelism level based on the current bandwidth
     * estimate. Higher bandwidths yield higher parallelism.
     *
     * @return recommended parallelism between 4 and 16
     */
    public int getRecommendedParallelism() {
        long bps = estimatedBandwidthBps;
        if (bps <= 0) {
            return 4;
        }
        if (bps > 100_000_000L) {
            return 16;
        }
        if (bps > 50_000_000L) {
            return 12;
        }
        if (bps > 10_000_000L) {
            return 8;
        }
        return 4;
    }

    /**
     * Resets all counters and estimates to their initial state.
     */
    public void reset() {
        totalBytesTransferred.set(0);
        totalDurationMs.set(0);
        currentWindowBytes = 0;
        currentWindowStartMs = 0;
        estimatedBandwidthBps = 0;
    }

    // --- Internal ---

    private void updateEstimate(long newBps) {
        if (estimatedBandwidthBps == 0) {
            estimatedBandwidthBps = newBps;
        } else {
            estimatedBandwidthBps = (estimatedBandwidthBps * 7 + newBps * 3) / 10;
        }
        log.trace("Bandwidth estimate: {} bps", estimatedBandwidthBps);
    }

    // --- Object ---

    @Override
    public String toString() {
        return String.format("BandwidthProfiler[estimate=%d bps, avg=%d bps, totalBytes=%d]",
                estimatedBandwidthBps, getAverageBandwidthBps(), totalBytesTransferred.get());
    }
}
