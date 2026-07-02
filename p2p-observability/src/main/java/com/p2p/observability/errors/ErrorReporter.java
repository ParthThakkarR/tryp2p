package com.p2p.observability.errors;

import com.p2p.core.model.ChunkInfo;
import com.p2p.core.model.FileMetadata;
import com.p2p.core.model.PeerId;

import java.time.Instant;

/**
 * Strategy interface for reporting errors that occur during P2P transfers.
 * Implementations may log to a file, console, Sentry, or any other sink.
 */
public interface ErrorReporter {

    // --- Public API ---

    /**
     * Reports a single error with full context.
     *
     * @param context the error context (must not be null per implementing classes)
     */
    void reportError(ErrorContext context);

    // --- Value types ---

    /**
     * Captures the full context surrounding an error event.
     *
     * @param timestamp  when the error occurred
     * @param peerId     the remote peer involved (may be null)
     * @param sessionId  the transfer session identifier (may be null)
     * @param errorType  a short classification label (e.g. "PROTOCOL_ERROR")
     * @param message    a human-readable description
     * @param stackTrace the exception stack trace (may be null)
     * @param fileInfo   the file being transferred (may be null)
     * @param chunkInfo  the chunk being processed (may be null)
     */
    record ErrorContext(
            Instant timestamp,
            PeerId peerId,
            String sessionId,
            String errorType,
            String message,
            String stackTrace,
            FileMetadata fileInfo,
            ChunkInfo chunkInfo
    ) {}
}
