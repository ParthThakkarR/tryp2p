package com.p2p.core.model;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.DisplayName;

import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.*;

@DisplayName("FileMetadata Tests")
class FileMetadataTest {

    @Test
    @DisplayName("calculates total chunks correctly")
    void calculatesChunks() {
        FileMetadata meta = FileMetadata.builder()
                .fileName("test.txt")
                .relativePath("test.txt")
                .fileSize(3 * 1024 * 1024 + 500) // 3 MB + 500 bytes
                .chunkSize(1024 * 1024) // 1 MB
                .build();

        assertEquals(4, meta.getTotalChunks()); // 3 full + 1 partial
    }

    @Test
    @DisplayName("zero-size file has zero chunks")
    void zeroSizeZeroChunks() {
        FileMetadata meta = FileMetadata.builder()
                .fileName("empty.txt")
                .relativePath("empty.txt")
                .fileSize(0)
                .chunkSize(1024 * 1024)
                .build();

        assertEquals(0, meta.getTotalChunks());
    }

    @Test
    @DisplayName("exact chunk boundary has correct count")
    void exactChunkBoundary() {
        FileMetadata meta = FileMetadata.builder()
                .fileName("exact.bin")
                .relativePath("exact.bin")
                .fileSize(3 * 1024 * 1024) // exactly 3 MB
                .chunkSize(1024 * 1024) // 1 MB
                .build();

        assertEquals(3, meta.getTotalChunks());
    }

    @Test
    @DisplayName("directory has zero chunks")
    void directoryZeroChunks() {
        FileMetadata meta = FileMetadata.builder()
                .fileName("mydir")
                .relativePath("mydir")
                .fileSize(0)
                .directory(true)
                .build();

        assertTrue(meta.isDirectory());
        assertEquals(0, meta.getTotalChunks());
    }

    @Test
    @DisplayName("isCompressible detects incompressible extensions")
    void compressibilityDetection() {
        assertFalse(FileMetadata.isCompressible(Path.of("photo.jpg")));
        assertFalse(FileMetadata.isCompressible(Path.of("archive.zip")));
        assertFalse(FileMetadata.isCompressible(Path.of("video.mp4")));
        assertFalse(FileMetadata.isCompressible(Path.of("archive.7z")));
        assertFalse(FileMetadata.isCompressible(Path.of("doc.docx")));
        assertTrue(FileMetadata.isCompressible(Path.of("source.java")));
        assertTrue(FileMetadata.isCompressible(Path.of("data.csv")));
        assertTrue(FileMetadata.isCompressible(Path.of("readme.md")));
        assertTrue(FileMetadata.isCompressible(Path.of("noextension")));
    }

    @Test
    @DisplayName("formattedSize returns human-readable strings")
    void formattedSize() {
        assertEquals("500 B", FileMetadata.builder()
                .fileName("a").relativePath("a").fileSize(500).chunkSize(1024).build()
                .getFormattedSize());

        String kbSize = FileMetadata.builder()
                .fileName("a").relativePath("a").fileSize(1536).chunkSize(1024).build()
                .getFormattedSize();
        assertTrue(kbSize.contains("KB"));

        String mbSize = FileMetadata.builder()
                .fileName("a").relativePath("a").fileSize(5 * 1024 * 1024).chunkSize(1024 * 1024).build()
                .getFormattedSize();
        assertTrue(mbSize.contains("MB"));
    }

    @Test
    @DisplayName("rejects negative file size")
    void rejectsNegativeSize() {
        assertThrows(IllegalArgumentException.class, () -> FileMetadata.builder()
                .fileName("bad.txt")
                .relativePath("bad.txt")
                .fileSize(-1)
                .chunkSize(1024)
                .build());
    }

    @Test
    @DisplayName("rejects zero chunk size for non-directory")
    void rejectsZeroChunkSize() {
        assertThrows(IllegalArgumentException.class, () -> FileMetadata.builder()
                .fileName("bad.txt")
                .relativePath("bad.txt")
                .fileSize(100)
                .chunkSize(0)
                .build());
    }
}
