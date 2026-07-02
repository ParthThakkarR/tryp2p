package com.p2p.transfer.engine;

import com.p2p.core.model.*;
import com.p2p.core.util.FileUtils;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.io.RandomAccessFile;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Objects;
import java.util.concurrent.ConcurrentSkipListSet;

/**
 * Writes chunks to a file at random-access positions, writing directly to the
 * target path (no {@code .p2p-tmp} intermediate). Thread-safe for concurrent
 * writes via synchronized blocks on the underlying file handle.
 */
public final class ChunkedFileWriter implements AutoCloseable {

    // --- Constants ---

    private static final Logger log = LoggerFactory.getLogger(ChunkedFileWriter.class);

    // --- Fields ---

    private final Path targetPath;
    private final FileMetadata metadata;
    private final RandomAccessFile raf;
    private final ConcurrentSkipListSet<Long> writtenChunks;

    // --- Constructor ---

    /**
     * Opens or creates the target file and pre-allocates space for the full
     * transfer.
     *
     * @param targetPath the final output path; parent directories are created
     * @param metadata   metadata describing chunk size and file layout
     * @throws IOException          if the file cannot be created or sized
     * @throws NullPointerException if either argument is null
     */
    public ChunkedFileWriter(Path targetPath, FileMetadata metadata) throws IOException {
        this.targetPath = Objects.requireNonNull(targetPath, "targetPath must not be null");
        this.metadata = Objects.requireNonNull(metadata, "metadata must not be null");

        FileUtils.ensureParentDirs(targetPath);
        this.raf = new RandomAccessFile(targetPath.toFile(), "rw");
        raf.setLength(metadata.getFileSize());

        this.writtenChunks = new ConcurrentSkipListSet<>();
    }

    // --- Public API ---

    /**
     * Writes a chunk at the position derived from its index. Optionally
     * verifies the data against the expected SHA-256 hash.
     *
     * @param chunkIndex   zero-based chunk index
     * @param data         chunk payload
     * @param expectedHash optional expected SHA-256 hash; may be null or empty
     *                     to skip verification
     * @return true if the chunk was written successfully (and hash matched,
     *         if provided)
     * @throws IOException              if the write fails
     * @throws IllegalArgumentException if chunkIndex is negative or data is
     *                                  empty
     * @throws NullPointerException     if data is null
     */
    public boolean writeChunk(long chunkIndex, byte[] data, String expectedHash) throws IOException {
        if (chunkIndex < 0) {
            throw new IllegalArgumentException("chunkIndex must be non-negative: " + chunkIndex);
        }
        Objects.requireNonNull(data, "data must not be null");
        if (data.length == 0) {
            throw new IllegalArgumentException("data must not be empty for chunk " + chunkIndex);
        }

        if (expectedHash != null && !expectedHash.isEmpty()) {
            String actualHash = FileUtils.sha256(data);
            if (!expectedHash.equals(actualHash)) {
                log.warn("Chunk {} hash mismatch: expected={}, actual={}",
                        chunkIndex, expectedHash, actualHash);
                return false;
            }
        }

        long offset = chunkIndex * metadata.getChunkSize();
        synchronized (raf) {
            raf.seek(offset);
            raf.write(data);
        }

        writtenChunks.add(chunkIndex);
        log.trace("Wrote chunk {} ({} bytes at offset {})", chunkIndex, data.length, offset);
        return true;
    }

    /**
     * Finalises the transfer: closes the file handle and verifies that all
     * expected chunks have been written.
     *
     * @throws IOException if the number of written chunks does not match the
     *                     expected total, or if the file cannot be closed
     */
    public void complete() throws IOException {
        raf.close();

        long written = writtenChunks.size();
        long total = metadata.getTotalChunks();
        if (written != total) {
            throw new IOException("Transfer incomplete: " + written + "/" + total + " chunks written");
        }

        log.info("Transfer complete: {}", targetPath);
    }

    /**
     * Aborts the transfer: closes the file handle and deletes the partially
     * written file.
     */
    public void abort() {
        try {
            raf.close();
        } catch (IOException ignored) {
            // best-effort
        }
        FileUtils.safeDelete(targetPath);
    }

    /**
     * Returns a snapshot copy of the set of written chunk indices.
     *
     * @return a new ConcurrentSkipListSet containing the indices of written chunks
     */
    public ConcurrentSkipListSet<Long> getWrittenChunks() {
        return new ConcurrentSkipListSet<>(writtenChunks);
    }

    /**
     * Returns whether a specific chunk has already been written.
     *
     * @param chunkIndex the chunk index to check
     * @return true if the chunk has been written
     */
    public boolean isChunkWritten(long chunkIndex) {
        return writtenChunks.contains(chunkIndex);
    }

    /**
     * Returns the number of chunks written so far.
     *
     * @return the count of written chunks
     */
    public long getWrittenCount() {
        return writtenChunks.size();
    }

    // --- Object ---

    @Override
    public void close() throws IOException {
        raf.close();
    }

    @Override
    public String toString() {
        return String.format("ChunkedFileWriter[targetPath=%s, written=%d/%d]",
                targetPath.getFileName(), writtenChunks.size(), metadata.getTotalChunks());
    }
}
