package com.p2p.transfer.engine;

import com.p2p.core.model.*;
import com.p2p.core.util.FileUtils;
import com.p2p.transfer.compression.DeflateCompressionService;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.io.TempDir;
import org.junit.jupiter.api.BeforeEach;

import java.io.IOException;
import java.nio.file.*;
import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

@DisplayName("Transfer Engine Tests")
class TransferEngineTest {

    @TempDir Path tempDir;
    private TransferEngine engine;

    @BeforeEach
    void setUp() {
        engine = new TransferEngine(4, 4);
    }

    @Test
    @DisplayName("createMetadata computes correct values for file")
    void createMetadataFile() throws Exception {
        Path file = tempDir.resolve("test.txt");
        byte[] content = "Hello, World! This is test content for P2P transfer.".getBytes();
        Files.write(file, content);

        FileMetadata meta = engine.createMetadata(file, 1024);
        assertEquals("test.txt", meta.getFileName());
        assertEquals(content.length, meta.getFileSize());
        assertFalse(meta.isDirectory());
        assertTrue(meta.isCompressible());
        assertNotNull(meta.getSha256Hash());
    }

    @Test
    @DisplayName("createMetadata identifies directories")
    void createMetadataDir() throws Exception {
        Path dir = tempDir.resolve("mydir");
        Files.createDirectories(dir);

        FileMetadata meta = engine.createMetadata(dir, 1024);
        assertTrue(meta.isDirectory());
        assertEquals(0, meta.getTotalChunks());
    }

    @Test
    @DisplayName("chunked read and write round-trip")
    void chunkedReadWriteRoundTrip() throws Exception {
        byte[] content = new byte[3 * 1024 + 500];
        for (int i = 0; i < content.length; i++) content[i] = (byte) (i % 256);
        Path srcFile = tempDir.resolve("source.bin");
        Files.write(srcFile, content);

        long chunkSize = 1024;
        FileMetadata meta = engine.createMetadata(srcFile, chunkSize);
        assertEquals(4, meta.getTotalChunks());

        Path dstFile = tempDir.resolve("dest.bin");
        try (ChunkedFileReader reader = new ChunkedFileReader(srcFile, meta);
             ChunkedFileWriter writer = new ChunkedFileWriter(dstFile, meta)) {

            for (long i = 0; i < meta.getTotalChunks(); i++) {
                ChunkedFileReader.ChunkData chunk = reader.readChunkWithHash(i);
                boolean ok = writer.writeChunk(i, chunk.data(), chunk.info().getSha256Hash());
                assertTrue(ok, "Chunk " + i + " should verify");
            }

            writer.complete();
        }

        assertArrayEquals(content, Files.readAllBytes(dstFile));
    }

    @Test
    @DisplayName("writer detects hash mismatch")
    void writerDetectsHashMismatch() throws Exception {
        byte[] content = new byte[1024];
        Path srcFile = tempDir.resolve("source.bin");
        Files.write(srcFile, content);

        FileMetadata meta = FileMetadata.builder()
                .fileName("source.bin").relativePath("source.bin")
                .fileSize(1024).chunkSize(1024).build();

        Path dstFile = tempDir.resolve("dest.bin");
        try (ChunkedFileWriter writer = new ChunkedFileWriter(dstFile, meta)) {
            boolean result = writer.writeChunk(0, content, "badhash");
            assertFalse(result, "Should reject bad hash");
        }
    }

    @Test
    @DisplayName("compression round-trip is lossless")
    void compressionRoundTrip() {
        DeflateCompressionService compression = new DeflateCompressionService();
        byte[] original = "This text should compress well because it has repeated patterns. "
                .repeat(100).getBytes();

        byte[] compressed = compression.compress(original);
        assertTrue(compressed.length < original.length, "Should compress");

        byte[] decompressed = compression.decompress(compressed);
        assertArrayEquals(original, decompressed);
    }

    @Test
    @DisplayName("compression estimation detects compressible data")
    void compressionEstimation() {
        DeflateCompressionService compression = new DeflateCompressionService();

        byte[] text = "AAAA".repeat(10000).getBytes();
        assertTrue(compression.estimateCompressionRatio(text) < 0.5);

        byte[] random = new byte[10000];
        new java.security.SecureRandom().nextBytes(random);
        assertTrue(compression.estimateCompressionRatio(random) > 0.9);
    }

    @Test
    @DisplayName("prepare and process chunk pipeline")
    void prepareAndProcessChunk() throws Exception {
        byte[] content = "Test data for chunk pipeline".repeat(100).getBytes();
        Path srcFile = tempDir.resolve("pipeline.txt");
        Files.write(srcFile, content);

        FileMetadata meta = engine.createMetadata(srcFile, content.length);
        Path dstFile = tempDir.resolve("pipeline-out.txt");

        try (ChunkedFileReader reader = new ChunkedFileReader(srcFile, meta);
             ChunkedFileWriter writer = new ChunkedFileWriter(dstFile, meta)) {

            TransferEngine.PreparedChunk prepared = engine.prepareChunk(
                    reader, 0, null, true);

            boolean ok = engine.processReceivedChunk(
                    writer, 0, prepared.data(), prepared.originalLength(),
                    prepared.compressed(), prepared.hash(), null);

            assertTrue(ok);
            writer.complete();
        }

        assertArrayEquals(content, Files.readAllBytes(dstFile));
    }

    @Test
    @DisplayName("directory scanning finds all entries")
    void directoryScan() throws Exception {
        Path dir = tempDir.resolve("scantest");
        Files.createDirectories(dir.resolve("sub1"));
        Files.createDirectories(dir.resolve("sub2/nested"));
        Files.write(dir.resolve("root.txt"), "root".getBytes());
        Files.write(dir.resolve("sub1/a.txt"), "a".getBytes());
        Files.write(dir.resolve("sub2/nested/b.txt"), "b".getBytes());

        List<FileMetadata> entries = engine.scanDirectory(dir, 1024);

        long dirs = entries.stream().filter(FileMetadata::isDirectory).count();
        long files = entries.stream().filter(e -> !e.isDirectory()).count();

        assertEquals(3, dirs);
        assertEquals(3, files);
    }
}
