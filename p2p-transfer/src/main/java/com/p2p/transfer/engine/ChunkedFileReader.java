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

/**
 * Reads chunks from a file using a {@link RandomAccessFile} for random-access
 * chunk retrieval. Thread-safe for concurrent reads via synchronized blocks on
 * the underlying file handle.
 */
public final class ChunkedFileReader implements AutoCloseable {

    // --- Constants ---

    private static final Logger log = LoggerFactory.getLogger(ChunkedFileReader.class);

    // --- Fields ---

    private final Path filePath;
    private final FileMetadata metadata;
    private final RandomAccessFile raf;

    // --- Constructor ---

    /**
     * Opens the specified file for chunked reading.
     *
     * @param filePath path to the file to read; must be a readable regular file
     * @param metadata metadata describing chunk size and file layout
     * @throws IOException          if the file cannot be opened for reading
     * @throws NullPointerException if either argument is null
     */
    public ChunkedFileReader(Path filePath, FileMetadata metadata) throws IOException {
        this.filePath = Objects.requireNonNull(filePath, "filePath must not be null");
        this.metadata = Objects.requireNonNull(metadata, "metadata must not be null");
        if (!Files.isRegularFile(filePath)) {
            throw new IllegalArgumentException("Not a regular file: " + filePath);
        }
        this.raf = new RandomAccessFile(filePath.toFile(), "r");
    }

    // --- Public API ---

    /**
     * Reads a single chunk from the file at the position computed from the
     * chunk index and the configured chunk size.
     *
     * @param chunkIndex the zero-based chunk index
     * @return the chunk data as a byte array
     * @throws IOException              if the chunk index is out of range or an I/O error occurs
     * @throws IllegalArgumentException if chunkIndex is negative
     */
    public byte[] readChunk(long chunkIndex) throws IOException {
        if (chunkIndex < 0) {
            throw new IllegalArgumentException("chunkIndex must be non-negative: " + chunkIndex);
        }
        long offset = chunkIndex * metadata.getChunkSize();
        long remaining = metadata.getFileSize() - offset;
        if (remaining <= 0) {
            throw new IOException("Invalid chunk index " + chunkIndex + ": offset " + offset
                    + " beyond file size " + metadata.getFileSize());
        }
        int length = (int) Math.min(metadata.getChunkSize(), remaining);

        byte[] buffer = new byte[length];
        synchronized (raf) {
            raf.seek(offset);
            raf.readFully(buffer);
        }
        return buffer;
    }

    /**
     * Reads a chunk and computes its SHA-256 hash, returning both as a
     * {@link ChunkData} record.
     *
     * @param chunkIndex the zero-based chunk index
     * @return a ChunkData record containing the chunk info and raw bytes
     * @throws IOException              if reading fails
     * @throws IllegalArgumentException if chunkIndex is negative
     */
    public ChunkData readChunkWithHash(long chunkIndex) throws IOException {
        byte[] data = readChunk(chunkIndex);
        String hash = FileUtils.sha256(data);
        long offset = chunkIndex * metadata.getChunkSize();

        ChunkInfo info = ChunkInfo.builder()
                .chunkIndex(chunkIndex)
                .offset(offset)
                .length(data.length)
                .sha256Hash(hash)
                .build();

        return new ChunkData(info, data);
    }

    /**
     * Reads a sample from the beginning of the file (up to sampleSize bytes)
     * for compression estimation.
     *
     * @param sampleSize the maximum number of bytes to read
     * @return the sample data
     * @throws IOException if reading fails
     */
    public byte[] readSample(int sampleSize) throws IOException {
        if (sampleSize <= 0) {
            throw new IllegalArgumentException("sampleSize must be positive: " + sampleSize);
        }
        int size = (int) Math.min(sampleSize, metadata.getFileSize());
        byte[] sample = new byte[size];
        synchronized (raf) {
            raf.seek(0);
            raf.readFully(sample);
        }
        return sample;
    }

    // --- Object ---

    @Override
    public void close() throws IOException {
        raf.close();
    }

    /**
     * A chunk of file data together with its {@link ChunkInfo} metadata.
     */
    public record ChunkData(ChunkInfo info, byte[] data) {}
}
