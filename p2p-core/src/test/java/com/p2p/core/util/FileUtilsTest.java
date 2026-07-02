package com.p2p.core.util;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.io.TempDir;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.*;

@DisplayName("FileUtils Tests")
class FileUtilsTest {

    @TempDir
    Path tempDir;

    @Test
    @DisplayName("sha256 computes correct hash for known input")
    void sha256KnownInput() {
        // SHA-256 of empty string = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        String hash = FileUtils.sha256(new byte[0]);
        assertEquals("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855", hash);
    }

    @Test
    @DisplayName("sha256 of byte array matches file hash")
    void sha256ConsistentBetweenArrayAndFile() throws IOException {
        byte[] data = "Hello, P2P World!".getBytes();
        Path file = tempDir.resolve("test.txt");
        Files.write(file, data);

        assertEquals(FileUtils.sha256(data), FileUtils.sha256(file));
    }

    @Test
    @DisplayName("totalSize computes file size correctly")
    void totalSizeSingleFile() throws IOException {
        byte[] data = new byte[4096];
        Path file = tempDir.resolve("test.bin");
        Files.write(file, data);

        assertEquals(4096, FileUtils.totalSize(file));
    }

    @Test
    @DisplayName("totalSize computes directory size recursively")
    void totalSizeDirectory() throws IOException {
        Path subdir = tempDir.resolve("sub");
        Files.createDirectories(subdir);
        Files.write(tempDir.resolve("a.txt"), new byte[100]);
        Files.write(subdir.resolve("b.txt"), new byte[200]);

        assertEquals(300, FileUtils.totalSize(tempDir));
    }

    @Test
    @DisplayName("availableDiskSpace returns positive value")
    void diskSpacePositive() throws IOException {
        long space = FileUtils.availableDiskSpace(tempDir);
        assertTrue(space > 0);
    }

    @Test
    @DisplayName("hasSufficientDiskSpace works correctly")
    void sufficientDiskSpace() throws IOException {
        assertTrue(FileUtils.hasSufficientDiskSpace(tempDir, 1, 1.1));
        // Requesting more space than exists should fail
        assertFalse(FileUtils.hasSufficientDiskSpace(tempDir, Long.MAX_VALUE / 2, 1.1));
    }

    @Test
    @DisplayName("normalizePath converts backslashes")
    void normalizePathConverts() {
        assertEquals("a/b/c", FileUtils.normalizePath("a\\b\\c"));
        assertEquals("a/b/c", FileUtils.normalizePath("a/b/c"));
    }

    @Test
    @DisplayName("ensureParentDirs creates directories")
    void ensureParentDirs() throws IOException {
        Path deep = tempDir.resolve("a/b/c/file.txt");
        FileUtils.ensureParentDirs(deep);
        assertTrue(Files.isDirectory(tempDir.resolve("a/b/c")));
    }

    @Test
    @DisplayName("safeDelete deletes file without throwing")
    void safeDelete() throws IOException {
        Path file = tempDir.resolve("deleteme.txt");
        Files.write(file, new byte[0]);
        assertTrue(FileUtils.safeDelete(file));
        assertFalse(Files.exists(file));
        // Deleting non-existent file returns false, no exception
        assertFalse(FileUtils.safeDelete(file));
    }

    @Test
    @DisplayName("formatSize returns human-readable strings")
    void formatSize() {
        assertEquals("0 B", FileUtils.formatSize(0));
        assertEquals("512 B", FileUtils.formatSize(512));
        assertTrue(FileUtils.formatSize(1536).contains("KB"));
        assertTrue(FileUtils.formatSize(5 * 1024 * 1024).contains("MB"));
        assertTrue(FileUtils.formatSize(2L * 1024 * 1024 * 1024).contains("GB"));
    }
}
