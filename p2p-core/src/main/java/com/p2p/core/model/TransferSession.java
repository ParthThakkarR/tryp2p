package com.p2p.core.model;

import java.time.Duration;
import java.time.Instant;
import java.util.Objects;
import java.util.UUID;
import java.util.concurrent.ConcurrentSkipListSet;

/**
 * Tracks the state and progress of a single file transfer session.
 * Thread-safe — all mutable state is synchronized.
 */
public final class TransferSession {

    private final String sessionId;
    private final PeerId localPeerId;
    private final PeerId remotePeerId;
    private final FileMetadata fileMetadata;
    private final TransferDirection direction;
    private final Instant createdAt;

    private volatile TransferState state;
    private volatile Instant startedAt;
    private volatile Instant completedAt;
    private volatile long bytesTransferred;
    private volatile long chunksCompleted;
    private volatile String errorMessage;
    private final ConcurrentSkipListSet<Long> completedChunks;

    private TransferSession(Builder builder) {
        this.sessionId = builder.sessionId != null
                ? builder.sessionId : UUID.randomUUID().toString();
        this.localPeerId = Objects.requireNonNull(builder.localPeerId, "localPeerId required");
        this.remotePeerId = Objects.requireNonNull(builder.remotePeerId, "remotePeerId required");
        this.fileMetadata = Objects.requireNonNull(builder.fileMetadata, "fileMetadata required");
        this.direction = Objects.requireNonNull(builder.direction, "direction required");
        this.createdAt = Instant.now();
        this.state = TransferState.PENDING;
        this.bytesTransferred = 0;
        this.chunksCompleted = 0;
        this.completedChunks = new ConcurrentSkipListSet<>();
    }

    // --- Getters ---

    public String getSessionId() { return sessionId; }
    public PeerId getLocalPeerId() { return localPeerId; }
    public PeerId getRemotePeerId() { return remotePeerId; }
    public FileMetadata getFileMetadata() { return fileMetadata; }
    public TransferDirection getDirection() { return direction; }
    public Instant getCreatedAt() { return createdAt; }

    // --- Thread-safe state access ---

    public synchronized TransferState getState() { return state; }
    public synchronized Instant getStartedAt() { return startedAt; }
    public synchronized Instant getCompletedAt() { return completedAt; }
    public synchronized long getBytesTransferred() { return bytesTransferred; }
    public synchronized long getChunksCompleted() { return chunksCompleted; }
    public synchronized String getErrorMessage() { return errorMessage; }

    // --- State transitions ---

    /**
     * Transitions the session to the given state, enforcing the valid state machine.
     *
     * @param newState the target state
     * @throws IllegalStateException if the transition is invalid or from a terminal state
     */
    public synchronized void transitionTo(TransferState newState) {
        validateTransition(this.state, newState);
        this.state = newState;

        if (newState == TransferState.TRANSFERRING && startedAt == null) {
            this.startedAt = Instant.now();
        }
        if (newState.isTerminal()) {
            this.completedAt = Instant.now();
        }
    }

    /**
     * Marks a chunk as completed and updates byte/chunk counters.
     *
     * @param chunkIndex  the index of the completed chunk (must be non-negative)
     * @param bytesInChunk the number of bytes in the chunk
     * @throws IndexOutOfBoundsException if chunkIndex is negative
     */
    public synchronized void markChunkCompleted(long chunkIndex, int bytesInChunk) {
        if (chunkIndex < 0) {
            throw new IndexOutOfBoundsException("Chunk index must be non-negative: " + chunkIndex);
        }
        if (completedChunks.add(chunkIndex)) {
            chunksCompleted++;
            bytesTransferred += bytesInChunk;
        }
    }

    /**
     * Transitions the session to the FAILED state with a descriptive error message.
     *
     * @param message description of the failure
     */
    public synchronized void fail(String message) {
        this.errorMessage = message;
        transitionTo(TransferState.FAILED);
    }

    public synchronized ConcurrentSkipListSet<Long> getCompletedChunksSet() {
        return new ConcurrentSkipListSet<>(completedChunks);
    }

    public synchronized boolean isChunkCompleted(long chunkIndex) {
        return completedChunks.contains(chunkIndex);
    }

    // --- Progress computation ---

    /**
     * Computes a snapshot of current transfer progress.
     *
     * @return a TransferProgress with bytes, chunks, percentage, speed, ETA
     */
    public synchronized TransferProgress getProgress() {
        long totalChunks = fileMetadata.getTotalChunks();
        double percentage = totalChunks == 0 ? 100.0
                : (double) chunksCompleted / totalChunks * 100.0;

        long elapsedMs = startedAt != null
                ? Duration.between(startedAt, completedAt != null ? completedAt : Instant.now()).toMillis()
                : 0;

        double speedBytesPerSec = elapsedMs > 0
                ? (double) bytesTransferred / elapsedMs * 1000.0
                : 0;

        long etaMs = speedBytesPerSec > 0
                ? (long) ((fileMetadata.getFileSize() - bytesTransferred) / speedBytesPerSec * 1000)
                : -1;

        return new TransferProgress(
                sessionId, state, bytesTransferred, fileMetadata.getFileSize(),
                chunksCompleted, totalChunks,
                percentage, speedBytesPerSec, elapsedMs, etaMs
        );
    }

    // --- Validation ---

    private void validateTransition(TransferState from, TransferState to) {
        if (from.isTerminal()) {
            throw new IllegalStateException(
                    "Cannot transition from terminal state " + from + " to " + to);
        }
        if (to == TransferState.FAILED || to == TransferState.CANCELLED) {
            return;
        }
        boolean valid = switch (from) {
            case PENDING -> to == TransferState.HANDSHAKING;
            case HANDSHAKING -> to == TransferState.NEGOTIATING;
            case NEGOTIATING -> to == TransferState.TRANSFERRING;
            case TRANSFERRING -> to == TransferState.PAUSED
                    || to == TransferState.VERIFYING
                    || to == TransferState.INTERRUPTED
                    || to == TransferState.COMPLETED;
            case PAUSED -> to == TransferState.TRANSFERRING;
            case INTERRUPTED -> to == TransferState.HANDSHAKING;
            case VERIFYING -> to == TransferState.COMPLETED;
            default -> false;
        };
        if (!valid) {
            throw new IllegalStateException(
                    "Invalid state transition: " + from + " \u2192 " + to);
        }
    }

    @Override
    public String toString() {
        long totalChunks = fileMetadata.getTotalChunks();
        return String.format("TransferSession[id=%s, %s, %s \u2192 %s, state=%s, progress=%.1f%%]",
                sessionId.substring(0, 8), direction,
                localPeerId.toShortString(), remotePeerId.toShortString(),
                fileMetadata.getFileName(), state,
                totalChunks == 0 ? 100.0
                        : (double) chunksCompleted / totalChunks * 100.0);
    }

    public static Builder builder() {
        return new Builder();
    }

    public static final class Builder {
        private String sessionId;
        private PeerId localPeerId;
        private PeerId remotePeerId;
        private FileMetadata fileMetadata;
        private TransferDirection direction;

        public Builder sessionId(String sessionId) { this.sessionId = sessionId; return this; }
        public Builder localPeerId(PeerId localPeerId) { this.localPeerId = localPeerId; return this; }
        public Builder remotePeerId(PeerId remotePeerId) { this.remotePeerId = remotePeerId; return this; }
        public Builder fileMetadata(FileMetadata fileMetadata) { this.fileMetadata = fileMetadata; return this; }
        public Builder direction(TransferDirection direction) { this.direction = direction; return this; }

        public TransferSession build() {
            return new TransferSession(this);
        }
    }
}
