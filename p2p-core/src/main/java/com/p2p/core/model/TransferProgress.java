package com.p2p.core.model;

/**
 * Immutable snapshot of transfer progress at a point in time.
 * All fields are exposed via the canonical record accessor methods.
 */
public record TransferProgress(
        String sessionId,
        TransferState state,
        long bytesTransferred,
        long totalBytes,
        long chunksCompleted,
        long totalChunks,
        double percentComplete,
        double speedBytesPerSec,
        long elapsedMs,
        long etaMs
) {
    public String formattedSpeed() {
        if (speedBytesPerSec < 1024) return String.format("%.0f B/s", speedBytesPerSec);
        if (speedBytesPerSec < 1024 * 1024) return String.format("%.1f KB/s", speedBytesPerSec / 1024);
        if (speedBytesPerSec < 1024.0 * 1024 * 1024) return String.format("%.1f MB/s", speedBytesPerSec / (1024 * 1024));
        return String.format("%.2f GB/s", speedBytesPerSec / (1024.0 * 1024 * 1024));
    }

    public String formattedEta() {
        if (etaMs < 0) return "unknown";
        long seconds = etaMs / 1000;
        if (seconds < 60) return seconds + "s";
        if (seconds < 3600) return String.format("%dm %ds", seconds / 60, seconds % 60);
        return String.format("%dh %dm", seconds / 3600, (seconds % 3600) / 60);
    }
}
