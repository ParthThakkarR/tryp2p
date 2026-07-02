package com.p2p.core.util;

import java.io.IOException;
import java.io.InputStream;
import java.nio.file.*;
import java.nio.file.attribute.BasicFileAttributes;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.HexFormat;
import java.util.concurrent.atomic.AtomicLong;

/**
 * File system utilities for cross-platform file operations.
 * All methods are stateless and thread-safe.
 */
public final class FileUtils {

    private static final int HASH_BUFFER_SIZE = 8192;

    private FileUtils() {}

    // --- Hashing ---

    /**
     * Computes the SHA-256 hash of a file.
     *
     * @param path path to the file
     * @return lowercase hex-encoded SHA-256 hash
     * @throws IOException if the file cannot be read
     */
    public static String sha256(Path path) throws IOException {
        try {
            MessageDigest digest = MessageDigest.getInstance("SHA-256");
            try (InputStream is = Files.newInputStream(path)) {
                byte[] buffer = new byte[HASH_BUFFER_SIZE];
                int bytesRead;
                while ((bytesRead = is.read(buffer)) != -1) {
                    digest.update(buffer, 0, bytesRead);
                }
            }
            return HexFormat.of().formatHex(digest.digest());
        } catch (NoSuchAlgorithmException e) {
            throw new RuntimeException("SHA-256 not available", e);
        }
    }

    /**
     * Computes the SHA-256 hash of a byte array.
     *
     * @param data the data to hash
     * @return lowercase hex-encoded SHA-256 hash
     */
    public static String sha256(byte[] data) {
        try {
            MessageDigest digest = MessageDigest.getInstance("SHA-256");
            return HexFormat.of().formatHex(digest.digest(data));
        } catch (NoSuchAlgorithmException e) {
            throw new RuntimeException("SHA-256 not available", e);
        }
    }

    // --- Size & Disk Space ---

    /**
     * Returns the total size of a file or directory (recursive).
     *
     * @param path path to the file or directory
     * @return total size in bytes
     * @throws IOException if an I/O error occurs
     */
    public static long totalSize(Path path) throws IOException {
        if (Files.isRegularFile(path)) {
            return Files.size(path);
        }
        AtomicLong total = new AtomicLong(0);
        Files.walkFileTree(path, new SimpleFileVisitor<>() {
            @Override
            public FileVisitResult visitFile(Path file, BasicFileAttributes attrs) {
                total.addAndGet(attrs.size());
                return FileVisitResult.CONTINUE;
            }
        });
        return total.get();
    }

    /**
     * Returns the available disk space at the given path in bytes.
     *
     * @param path the path to check
     * @return available bytes
     * @throws IOException if an I/O error occurs
     */
    public static long availableDiskSpace(Path path) throws IOException {
        return Files.getFileStore(path.toRealPath()).getUsableSpace();
    }

    /**
     * Checks if there is sufficient disk space for a transfer.
     *
     * @param targetDir     the directory where the file will be saved
     * @param requiredBytes the required space in bytes
     * @param factor        safety factor (e.g., 1.1 for 110%)
     * @return true if sufficient space is available
     * @throws IOException if an I/O error occurs
     */
    public static boolean hasSufficientDiskSpace(Path targetDir, long requiredBytes, double factor)
            throws IOException {
        long available = availableDiskSpace(targetDir);
        return available >= (long) (requiredBytes * factor);
    }

    // --- Path Utilities ---

    /**
     * Normalizes a relative path for cross-platform compatibility.
     * Converts backslashes to forward slashes.
     *
     * @param path the path string to normalize
     * @return normalized path string
     */
    public static String normalizePath(String path) {
        return path.replace('\\', '/');
    }

    /**
     * Creates parent directories if they don't exist.
     *
     * @param path the file path whose parent directories should be created
     * @throws IOException if directories cannot be created
     */
    public static void ensureParentDirs(Path path) throws IOException {
        Path parent = path.getParent();
        if (parent != null && !Files.exists(parent)) {
            Files.createDirectories(parent);
        }
    }

    /**
     * Safely deletes a file if it exists, without throwing exceptions.
     *
     * @param path the file to delete
     * @return true if the file was deleted, false otherwise
     */
    public static boolean safeDelete(Path path) {
        try {
            return Files.deleteIfExists(path);
        } catch (IOException e) {
            return false;
        }
    }

    // --- Formatting ---

    /**
     * Returns a human-readable file size string (e.g., "1.5 MB").
     *
     * @param bytes the size in bytes
     * @return formatted size string
     */
    public static String formatSize(long bytes) {
        if (bytes < 1024) return bytes + " B";
        if (bytes < 1024 * 1024) return String.format("%.1f KB", bytes / 1024.0);
        if (bytes < 1024L * 1024 * 1024) return String.format("%.1f MB", bytes / (1024.0 * 1024));
        return String.format("%.2f GB", bytes / (1024.0 * 1024 * 1024));
    }
}
