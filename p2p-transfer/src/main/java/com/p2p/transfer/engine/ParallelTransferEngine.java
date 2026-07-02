package com.p2p.transfer.engine;

import com.p2p.core.config.AppConfig;
import com.p2p.core.exception.CryptoException;
import com.p2p.core.exception.ErrorCode;
import com.p2p.core.exception.TransferException;
import com.p2p.core.model.*;
import com.p2p.core.service.EncryptionService;
import com.p2p.core.util.FileUtils;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicLong;
import java.util.function.Consumer;

/**
 * Parallel file transfer engine that orchestrates concurrent chunk preparation
 * and processing using virtual threads. Supports sending and receiving files
 * with progress callbacks, bandwidth profiling, and adaptive chunk sizing.
 * Thread-safe for cancellation from any thread.
 */
public final class ParallelTransferEngine implements AutoCloseable {

    // --- Constants ---

    private static final Logger log = LoggerFactory.getLogger(ParallelTransferEngine.class);

    // --- Fields ---

    private final int maxParallelism;
    private final int maxInflightChunks;
    private final long chunkSize;
    private final TransferEngine singleEngine;
    private final AdaptiveChunkSizer chunkSizer;
    private final BandwidthProfiler bandwidthProfiler;
    private final TransferPriorityQueue<PendingChunk> priorityQueue;
    private final AtomicBoolean running = new AtomicBoolean(false);
    private final AtomicInteger activeWorkers = new AtomicInteger(0);
    private final AtomicLong totalBytesTransferred = new AtomicLong(0);
    private final ExecutorService workerPool;
    private final ScheduledExecutorService scheduler;

    // --- Constructor ---

    /**
     * Creates a ParallelTransferEngine configured from the given application
     * config.
     *
     * @param config application configuration providing parallelism limits,
     *               chunk size, and inflight limits
     * @throws NullPointerException if config is null
     */
    public ParallelTransferEngine(AppConfig config) {
        Objects.requireNonNull(config, "config must not be null");
        this.maxParallelism = Math.min(config.getParallelism(), config.getMaxParallelism());
        this.maxInflightChunks = config.getMaxInflightChunks();
        this.chunkSize = config.getChunkSize();
        this.singleEngine = new TransferEngine(maxParallelism, maxInflightChunks);
        this.chunkSizer = new AdaptiveChunkSizer(chunkSize);
        this.bandwidthProfiler = new BandwidthProfiler();
        this.priorityQueue = new TransferPriorityQueue<>();
        this.workerPool = Executors.newVirtualThreadPerTaskExecutor();
        this.scheduler = Executors.newScheduledThreadPool(2, r -> {
            Thread t = new Thread(r, "parallel-xfer-scheduler");
            t.setDaemon(true);
            return t;
        });
    }

    // --- Public API ---

    /**
     * Sends a file to a remote peer using parallel chunk processing. Each
     * chunk is read, optionally compressed and encrypted, then tracked for
     * progress reporting.
     *
     * @param filePath           the file to send
     * @param remotePeer         the destination peer info (unused in current impl)
     * @param encryptionService  optional encryption service; may be null
     * @param shouldCompress     whether to attempt compression
     * @param progressCallback   optional callback invoked per chunk with progress
     * @return a TransferResult summarising the transfer outcome
     * @throws TransferException if the file cannot be read or the transfer fails
     * @throws NullPointerException if filePath is null
     */
    public TransferResult sendFile(Path filePath, PeerInfo remotePeer,
                                    EncryptionService encryptionService,
                                    boolean shouldCompress,
                                    Consumer<TransferProgress> progressCallback) throws TransferException {
        Objects.requireNonNull(filePath, "filePath must not be null");
        try {
            FileMetadata metadata = singleEngine.createMetadata(filePath, chunkSizer.getCurrentChunkSize());
            return sendFileWithMetadata(metadata, filePath, remotePeer, encryptionService,
                    shouldCompress, progressCallback);
        } catch (IOException e) {
            throw new TransferException(ErrorCode.TRANSFER_FAILED,
                    "Failed to read file: " + filePath, e);
        }
    }

    /**
     * Sends a file with an already-resolved {@link FileMetadata}.
     *
     * @param metadata           pre-computed file metadata
     * @param filePath           the file to send
     * @param remotePeer         the destination peer
     * @param encryptionService  optional encryption; may be null
     * @param shouldCompress     whether to attempt compression
     * @param progressCallback   optional progress callback
     * @return a TransferResult summarising the transfer outcome
     * @throws TransferException if the transfer fails
     * @throws NullPointerException if metadata or filePath is null
     */
    public TransferResult sendFileWithMetadata(FileMetadata metadata, Path filePath,
                                                PeerInfo remotePeer,
                                                EncryptionService encryptionService,
                                                boolean shouldCompress,
                                                Consumer<TransferProgress> progressCallback) throws TransferException {
        Objects.requireNonNull(metadata, "metadata must not be null");
        Objects.requireNonNull(filePath, "filePath must not be null");
        if (!Files.isRegularFile(filePath) || !Files.isReadable(filePath)) {
            throw new TransferException(ErrorCode.TRANSFER_FAILED,
                    "File not readable: " + filePath);
        }

        running.set(true);
        totalBytesTransferred.set(0);

        try (ChunkedFileReader reader = new ChunkedFileReader(filePath, metadata)) {
            long totalChunks = metadata.getTotalChunks();
            CountDownLatch completionLatch = new CountDownLatch((int) totalChunks);
            AtomicInteger failedChunks = new AtomicInteger(0);

            long startTime = System.currentTimeMillis();

            for (long i = 0; i < totalChunks && running.get(); i++) {
                long chunkIndex = i;
                workerPool.submit(() -> {
                    try {
                        long chunkStart = System.currentTimeMillis();
                        TransferEngine.PreparedChunk prepared = singleEngine.prepareChunk(
                                reader, chunkIndex, encryptionService, shouldCompress);

                        long chunkDuration = System.currentTimeMillis() - chunkStart;
                        totalBytesTransferred.addAndGet(prepared.data().length);
                        bandwidthProfiler.recordTransfer(prepared.data().length, chunkDuration);

                        if (progressCallback != null) {
                            long bytesTransferred = totalBytesTransferred.get();
                            double speed = calculateSpeed(bytesTransferred, startTime);
                            progressCallback.accept(new TransferProgress(
                                    metadata.getFileName(), TransferState.TRANSFERRING,
                                    bytesTransferred, metadata.getFileSize(),
                                    chunkIndex + 1, totalChunks,
                                    (double) (chunkIndex + 1) / totalChunks * 100.0,
                                    speed,
                                    System.currentTimeMillis() - startTime,
                                    calculateETA(bytesTransferred, metadata.getFileSize(), speed)
                            ));
                        }
                    } catch (Exception e) {
                        failedChunks.incrementAndGet();
                        log.error("Chunk {} failed: {}", chunkIndex, e.getMessage(), e);
                    } finally {
                        completionLatch.countDown();
                    }
                });
            }

            completionLatch.await();

            long duration = System.currentTimeMillis() - startTime;
            boolean success = failedChunks.get() == 0 && running.get();

            return new TransferResult(success, metadata.getFileSize(), duration,
                    totalBytesTransferred.get(), failedChunks.get(),
                    success ? null : "Transfer failed with " + failedChunks.get() + " chunk errors");
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            throw new TransferException(ErrorCode.TRANSFER_FAILED,
                    "Transfer interrupted", e);
        } catch (IOException e) {
            throw new TransferException(ErrorCode.TRANSFER_FAILED,
                    "Transfer failed", e);
        } finally {
            running.set(false);
        }
    }

    /**
     * Receives a file by processing received chunks in parallel. After all
     * chunks are processed, either completes or aborts the writer.
     *
     * @param writer            target file writer
     * @param metadata          expected file metadata
     * @param encryptionService optional encryption; may be null
     * @param chunks            map of chunk index to received chunk data
     * @param progressCallback  optional progress callback
     * @return a TransferResult summarising the receive outcome
     * @throws TransferException     if the receive fails
     * @throws NullPointerException  if writer, metadata, or chunks is null
     */
    public TransferResult receiveFile(ChunkedFileWriter writer, FileMetadata metadata,
                                       EncryptionService encryptionService,
                                       Map<Long, ReceivedChunk> chunks,
                                       Consumer<TransferProgress> progressCallback) throws TransferException {
        Objects.requireNonNull(writer, "writer must not be null");
        Objects.requireNonNull(metadata, "metadata must not be null");
        Objects.requireNonNull(chunks, "chunks must not be null");

        running.set(true);
        totalBytesTransferred.set(0);

        try {
            long totalChunks = metadata.getTotalChunks();
            CountDownLatch completionLatch = new CountDownLatch((int) totalChunks);
            AtomicInteger failedChunks = new AtomicInteger(0);
            long startTime = System.currentTimeMillis();

            for (long i = 0; i < totalChunks && running.get(); i++) {
                long chunkIndex = i;
                ReceivedChunk chunk = chunks.get(chunkIndex);
                if (chunk == null) {
                    failedChunks.incrementAndGet();
                    completionLatch.countDown();
                    continue;
                }

                workerPool.submit(() -> {
                    try {
                        long chunkStart = System.currentTimeMillis();
                        boolean ok = singleEngine.processReceivedChunk(
                                writer, chunkIndex, chunk.data(), chunk.originalLength(),
                                chunk.compressed(), chunk.hash(), encryptionService);

                        if (ok) {
                            totalBytesTransferred.addAndGet(chunk.data().length);
                            bandwidthProfiler.recordTransfer(chunk.data().length,
                                    System.currentTimeMillis() - chunkStart);
                        } else {
                            failedChunks.incrementAndGet();
                            log.warn("Chunk {} verification failed", chunkIndex);
                        }

                        if (progressCallback != null) {
                            long bytesTransferred = totalBytesTransferred.get();
                            double speed = calculateSpeed(bytesTransferred, startTime);
                            progressCallback.accept(new TransferProgress(
                                    metadata.getFileName(), TransferState.TRANSFERRING,
                                    bytesTransferred, metadata.getFileSize(),
                                    chunkIndex + 1, totalChunks,
                                    (double) (chunkIndex + 1) / totalChunks * 100.0,
                                    speed,
                                    System.currentTimeMillis() - startTime,
                                    calculateETA(bytesTransferred, metadata.getFileSize(), speed)
                            ));
                        }
                    } catch (Exception e) {
                        failedChunks.incrementAndGet();
                        log.error("Receive chunk {} failed: {}", chunkIndex, e.getMessage(), e);
                    } finally {
                        completionLatch.countDown();
                    }
                });
            }

            completionLatch.await();

            long duration = System.currentTimeMillis() - startTime;
            boolean success = failedChunks.get() == 0 && running.get();

            if (success) {
                writer.complete();
            } else {
                writer.abort();
            }

            return new TransferResult(success, metadata.getFileSize(), duration,
                    totalBytesTransferred.get(), failedChunks.get(),
                    success ? null : "Receive failed with " + failedChunks.get() + " chunk errors");
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            throw new TransferException(ErrorCode.TRANSFER_FAILED,
                    "Receive interrupted", e);
        } catch (IOException e) {
            throw new TransferException(ErrorCode.TRANSFER_FAILED,
                    "Receive failed", e);
        } finally {
            running.set(false);
        }
    }

    // --- Control ---

    /**
     * Cancels the current transfer. Safe to call from any thread.
     */
    public void cancel() {
        running.set(false);
    }

    /**
     * Pauses the current transfer (same as cancel in current implementation).
     */
    public void pause() {
        running.set(false);
    }

    /**
     * Resumes a paused transfer.
     */
    public void resume() {
        running.set(true);
    }

    /**
     * Returns whether a transfer is currently running.
     *
     * @return true if actively transferring
     */
    public boolean isRunning() {
        return running.get();
    }

    /**
     * Returns the number of active worker tasks.
     *
     * @return active worker count
     */
    public int getActiveWorkers() {
        return activeWorkers.get();
    }

    /**
     * Returns the total number of bytes transferred so far.
     *
     * @return total bytes transferred
     */
    public long getTotalBytesTransferred() {
        return totalBytesTransferred.get();
    }

    /**
     * Returns the bandwidth profiler for this engine.
     *
     * @return the bandwidth profiler instance
     */
    public BandwidthProfiler getBandwidthProfiler() {
        return bandwidthProfiler;
    }

    /**
     * Returns the adaptive chunk sizer for this engine.
     *
     * @return the chunk sizer instance
     */
    public AdaptiveChunkSizer getChunkSizer() {
        return chunkSizer;
    }

    // --- Object ---

    @Override
    public void close() {
        cancel();
        workerPool.shutdownNow();
        scheduler.shutdownNow();
        try {
            workerPool.awaitTermination(5, TimeUnit.SECONDS);
        } catch (InterruptedException ignored) {
            Thread.currentThread().interrupt();
        }
    }

    // --- Internal ---

    private double calculateSpeed(long bytes, long startTimeMs) {
        long elapsed = System.currentTimeMillis() - startTimeMs;
        return elapsed > 0 ? (double) bytes / elapsed * 1000.0 : 0;
    }

    private long calculateETA(long transferred, long total, double speedBps) {
        if (speedBps <= 0) {
            return -1;
        }
        return (long) ((total - transferred) / speedBps * 1000.0);
    }

    // --- Records ---

    /**
     * A pending chunk in the transfer priority queue.
     */
    public record PendingChunk(long chunkIndex, long offset, int length) {}

    /**
     * A chunk received from the network, with original metadata preserved.
     */
    public record ReceivedChunk(byte[] data, int originalLength, boolean compressed, String hash) {}

    /**
     * Summary result of a file transfer operation.
     */
    public record TransferResult(boolean success, long totalBytes, long durationMs,
                                  long bytesTransferred, int failedChunks, String errorMessage) {

        /**
         * Returns the transfer speed in bytes per second.
         *
         * @return speed in B/s, or 0 if duration is zero
         */
        public double getSpeedBytesPerSec() {
            return durationMs > 0 ? (double) bytesTransferred / durationMs * 1000.0 : 0;
        }

        /**
         * Returns a human-readable duration string.
         *
         * @return e.g. "5s", "2m 30s"
         */
        public String getFormattedDuration() {
            long seconds = durationMs / 1000;
            if (seconds < 60) {
                return seconds + "s";
            }
            long minutes = seconds / 60;
            seconds = seconds % 60;
            return minutes + "m " + seconds + "s";
        }

        @Override
        public String toString() {
            return String.format("TransferResult[success=%s, bytes=%d, durationMs=%d, errors=%d]",
                    success, bytesTransferred, durationMs, failedChunks);
        }
    }
}
