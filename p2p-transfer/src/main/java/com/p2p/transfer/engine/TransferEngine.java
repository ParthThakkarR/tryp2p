package com.p2p.transfer.engine;

import com.p2p.core.model.*;
import com.p2p.core.service.CompressionService;
import com.p2p.core.service.EncryptionService;
import com.p2p.core.exception.CryptoException;
import com.p2p.core.util.FileUtils;
import com.p2p.transfer.compression.DeflateCompressionService;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.file.*;
import java.nio.file.attribute.BasicFileAttributes;
import java.util.*;

/**
 * Orchestrates chunk preparation and processing for file transfers. Handles
 * metadata creation, optional compression with adaptive ratio estimation,
 * and optional AEAD encryption. Not thread-safe; intended to be used by a
 * single transfer flow at a time.
 */
public final class TransferEngine {

    // --- Constants ---

    private static final Logger log = LoggerFactory.getLogger(TransferEngine.class);

    private static final double COMPRESSION_THRESHOLD = 0.9;
    private static final int SAMPLE_SIZE = 65536;

    // --- Fields ---

    private final CompressionService compressionService;
    private final int maxParallelism;
    private final int maxInflightChunks;

    // --- Constructor ---

    /**
     * Creates a TransferEngine with the given concurrency limits.
     *
     * @param maxParallelism    maximum number of parallel chunk operations
     * @param maxInflightChunks maximum number of chunks in-flight at once
     */
    public TransferEngine(int maxParallelism, int maxInflightChunks) {
        if (maxParallelism < 1) {
            throw new IllegalArgumentException("maxParallelism must be >= 1: " + maxParallelism);
        }
        if (maxInflightChunks < 1) {
            throw new IllegalArgumentException("maxInflightChunks must be >= 1: " + maxInflightChunks);
        }
        this.compressionService = new DeflateCompressionService();
        this.maxParallelism = maxParallelism;
        this.maxInflightChunks = maxInflightChunks;
    }

    // --- Public API ---

    /**
     * Creates {@link FileMetadata} for a file or directory, computing the
     * SHA-256 hash and compression hint for regular files.
     *
     * @param path      the file or directory to describe
     * @param chunkSize the chunk size in bytes
     * @return populated FileMetadata
     * @throws IOException              if the path cannot be read
     * @throws NullPointerException     if path is null
     * @throws IllegalArgumentException if chunkSize is not positive
     */
    public FileMetadata createMetadata(Path path, long chunkSize) throws IOException {
        Objects.requireNonNull(path, "path must not be null");
        if (chunkSize <= 0) {
            throw new IllegalArgumentException("chunkSize must be positive: " + chunkSize);
        }
        if (!Files.exists(path)) {
            throw new IllegalArgumentException("Path does not exist: " + path);
        }

        boolean isDir = Files.isDirectory(path);

        FileMetadata.Builder builder = FileMetadata.builder()
                .fileName(path.getFileName().toString())
                .relativePath(FileUtils.normalizePath(path.getFileName().toString()))
                .directory(isDir)
                .chunkSize(chunkSize);

        if (isDir) {
            builder.fileSize(0);
        } else {
            long size = Files.size(path);
            String hash = FileUtils.sha256(path);
            boolean compressible = FileMetadata.isCompressible(path);

            builder.fileSize(size)
                    .sha256Hash(hash)
                    .lastModified(Files.getLastModifiedTime(path).toMillis())
                    .compressible(compressible);
        }

        return builder.build();
    }

    /**
     * Reads a chunk from the reader, optionally compresses it, then optionally
     * encrypts it. Returns a {@link PreparedChunk} ready for network transport.
     *
     * @param reader             source file reader
     * @param chunkIndex         the chunk to prepare
     * @param encryptionService  optional encryption service; may be null
     * @param shouldCompress     whether to attempt compression
     * @return a PreparedChunk with potentially compressed/encrypted data
     * @throws IOException          if reading fails
     * @throws CryptoException      if encryption fails
     * @throws NullPointerException if reader is null
     */
    public PreparedChunk prepareChunk(ChunkedFileReader reader, long chunkIndex,
                                       EncryptionService encryptionService,
                                       boolean shouldCompress) throws IOException, CryptoException {
        Objects.requireNonNull(reader, "reader must not be null");

        ChunkedFileReader.ChunkData raw = reader.readChunkWithHash(chunkIndex);
        byte[] data = raw.data();
        String hash = raw.info().getSha256Hash();
        boolean compressed = false;
        int originalLength = data.length;

        if (shouldCompress) {
            byte[] compressedData = compressionService.compress(data);
            if (compressedData.length < data.length * COMPRESSION_THRESHOLD) {
                data = compressedData;
                compressed = true;
            }
        }

        if (encryptionService != null) {
            data = encryptionService.encryptChunk(data, chunkIndex);
        }

        return new PreparedChunk(chunkIndex, raw.info().getOffset(), originalLength,
                compressed, hash, data);
    }

    /**
     * Decrypts (if needed), decompresses (if needed), and writes a received
     * chunk via the writer.
     *
     * @param writer             target file writer
     * @param chunkIndex         the chunk index
     * @param data               the received chunk payload
     * @param originalLength     original uncompressed length
     * @param isCompressed       whether the data was compressed before encryption
     * @param expectedHash       expected SHA-256 hash for verification
     * @param encryptionService  optional encryption service; may be null
     * @return true if the chunk was written and verified successfully
     * @throws IOException          if reading or writing fails
     * @throws CryptoException      if decryption fails
     * @throws NullPointerException if writer or data is null
     */
    public boolean processReceivedChunk(ChunkedFileWriter writer, long chunkIndex,
                                         byte[] data, int originalLength,
                                         boolean isCompressed, String expectedHash,
                                         EncryptionService encryptionService)
            throws IOException, CryptoException {

        Objects.requireNonNull(writer, "writer must not be null");
        Objects.requireNonNull(data, "data must not be null");

        byte[] processed = data;
        if (encryptionService != null) {
            processed = encryptionService.decryptChunk(processed, chunkIndex);
        }

        if (isCompressed) {
            processed = compressionService.decompress(processed);
        }

        return writer.writeChunk(chunkIndex, processed, expectedHash);
    }

    /**
     * Determines whether the file should be compressed based on its type,
     * size, and a sample estimate.
     *
     * @param reader   source file reader for sampling
     * @param metadata the file metadata
     * @return true if compression is likely beneficial
     * @throws IOException if sampling fails
     */
    public boolean shouldCompress(ChunkedFileReader reader, FileMetadata metadata) throws IOException {
        if (!metadata.isCompressible()) {
            return false;
        }
        if (metadata.getFileSize() < 1024) {
            return false;
        }

        byte[] sample = reader.readSample(SAMPLE_SIZE);
        double ratio = compressionService.estimateCompressionRatio(sample);
        return ratio < COMPRESSION_THRESHOLD;
    }

    /**
     * Scans a directory recursively and returns a list of FileMetadata entries
     * for every file and subdirectory.
     *
     * @param directory the root directory to scan
     * @param chunkSize the chunk size to record in each entry
     * @return list of FileMetadata entries, one per file and subdirectory
     * @throws IOException              if the scan fails
     * @throws NullPointerException     if directory is null
     * @throws IllegalArgumentException if chunkSize is not positive
     */
    public List<FileMetadata> scanDirectory(Path directory, long chunkSize) throws IOException {
        Objects.requireNonNull(directory, "directory must not be null");
        if (chunkSize <= 0) {
            throw new IllegalArgumentException("chunkSize must be positive: " + chunkSize);
        }
        if (!Files.isDirectory(directory)) {
            throw new IllegalArgumentException("Not a directory: " + directory);
        }

        List<FileMetadata> entries = new ArrayList<>();

        Files.walkFileTree(directory, new SimpleFileVisitor<>() {
            @Override
            public FileVisitResult preVisitDirectory(Path dir, BasicFileAttributes attrs) {
                String relative = FileUtils.normalizePath(directory.relativize(dir).toString());
                if (!relative.isEmpty()) {
                    entries.add(FileMetadata.builder()
                            .fileName(dir.getFileName().toString())
                            .relativePath(relative)
                            .directory(true)
                            .fileSize(0)
                            .chunkSize(chunkSize)
                            .build());
                }
                return FileVisitResult.CONTINUE;
            }

            @Override
            public FileVisitResult visitFile(Path file, BasicFileAttributes attrs) throws IOException {
                String relative = FileUtils.normalizePath(directory.relativize(file).toString());
                entries.add(FileMetadata.builder()
                        .fileName(file.getFileName().toString())
                        .relativePath(relative)
                        .fileSize(attrs.size())
                        .sha256Hash(FileUtils.sha256(file))
                        .lastModified(attrs.lastModifiedTime().toMillis())
                        .compressible(FileMetadata.isCompressible(file))
                        .chunkSize(chunkSize)
                        .build());
                return FileVisitResult.CONTINUE;
            }
        });

        return entries;
    }

    // --- Records ---

    /**
     * A chunk that has been prepared for transmission: optionally compressed
     * and encrypted, with original metadata preserved.
     */
    public record PreparedChunk(long chunkIndex, long offset, int originalLength,
                                 boolean compressed, String hash, byte[] data) {}
}
