package com.p2p.transfer.engine;

import com.p2p.core.model.FileMetadata;
import com.p2p.core.util.FileUtils;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.file.FileVisitResult;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.SimpleFileVisitor;
import java.nio.file.attribute.BasicFileAttributes;
import java.util.ArrayList;
import java.util.List;
import java.util.Objects;
import java.util.Set;

/**
 * Scans directories recursively to produce a list of {@link FileMetadata}
 * entries for each file and subdirectory. Skips common VCS and dependency
 * directories (.git, .svn, node_modules, etc.). Not thread-safe.
 */
public final class DirectoryScanner {

    // --- Constants ---

    private static final Logger log = LoggerFactory.getLogger(DirectoryScanner.class);

    private static final Set<String> SKIP_DIRS = Set.of(
            ".git", ".svn", ".hg", "__pycache__", "node_modules"
    );

    // --- Public API ---

    /**
     * Recursively scans a directory and returns metadata for every file and
     * subdirectory.
     *
     * @param directory the root directory to scan
     * @param chunkSize the chunk size to embed in each metadata entry
     * @return list of FileMetadata entries (directories first, then files)
     * @throws IOException              if the scan fails
     * @throws NullPointerException     if directory is null
     * @throws IllegalArgumentException if directory does not exist or is not a
     *                                  directory, or if chunkSize is not positive
     */
    public List<FileMetadata> scan(Path directory, long chunkSize) throws IOException {
        Objects.requireNonNull(directory, "directory must not be null");
        if (chunkSize <= 0) {
            throw new IllegalArgumentException("chunkSize must be positive: " + chunkSize);
        }
        if (!Files.isDirectory(directory)) {
            throw new IllegalArgumentException("Not a directory: " + directory);
        }

        List<FileMetadata> entries = new ArrayList<>();
        Path root = directory.toAbsolutePath();

        Files.walkFileTree(root, new SimpleFileVisitor<>() {
            @Override
            public FileVisitResult preVisitDirectory(Path dir, BasicFileAttributes attrs) {
                String dirName = dir.getFileName().toString();
                if (SKIP_DIRS.contains(dirName)) {
                    log.debug("Skipping directory: {}", dirName);
                    return FileVisitResult.SKIP_SUBTREE;
                }

                Path relative = root.relativize(dir);
                String relativePath = relative.toString().replace('\\', '/');

                if (!relative.equals(Path.of(""))) {
                    entries.add(FileMetadata.builder()
                            .fileName(dir.getFileName().toString())
                            .relativePath(relativePath)
                            .directory(true)
                            .fileSize(0)
                            .chunkSize(chunkSize)
                            .build());
                }

                return FileVisitResult.CONTINUE;
            }

            @Override
            public FileVisitResult visitFile(Path file, BasicFileAttributes attrs) throws IOException {
                Path relative = root.relativize(file);
                String relativePath = relative.toString().replace('\\', '/');

                entries.add(FileMetadata.builder()
                        .fileName(file.getFileName().toString())
                        .relativePath(relativePath)
                        .fileSize(attrs.size())
                        .sha256Hash(FileUtils.sha256(file))
                        .lastModified(attrs.lastModifiedTime().toMillis())
                        .compressible(FileMetadata.isCompressible(file))
                        .chunkSize(chunkSize)
                        .build());

                return FileVisitResult.CONTINUE;
            }

            @Override
            public FileVisitResult visitFileFailed(Path file, IOException exc) {
                log.warn("Failed to access file: {} - {}", file, exc.getMessage());
                return FileVisitResult.CONTINUE;
            }
        });

        log.info("Scanned directory {}: {} entries", directory, entries.size());
        return entries;
    }

    /**
     * Calculates the total size of all non-directory entries.
     *
     * @param entries the list of file metadata entries
     * @return total file size in bytes
     * @throws NullPointerException if entries is null
     */
    public long calculateTotalSize(List<FileMetadata> entries) {
        Objects.requireNonNull(entries, "entries must not be null");
        return entries.stream()
                .filter(e -> !e.isDirectory())
                .mapToLong(FileMetadata::getFileSize)
                .sum();
    }

    /**
     * Counts the number of regular files (non-directory entries).
     *
     * @param entries the list of file metadata entries
     * @return the file count
     * @throws NullPointerException if entries is null
     */
    public int countFiles(List<FileMetadata> entries) {
        Objects.requireNonNull(entries, "entries must not be null");
        return (int) entries.stream().filter(e -> !e.isDirectory()).count();
    }

    /**
     * Counts the number of directory entries.
     *
     * @param entries the list of file metadata entries
     * @return the directory count
     * @throws NullPointerException if entries is null
     */
    public int countDirectories(List<FileMetadata> entries) {
        Objects.requireNonNull(entries, "entries must not be null");
        return (int) entries.stream().filter(FileMetadata::isDirectory).count();
    }

    // --- Object ---

    @Override
    public String toString() {
        return String.format("DirectoryScanner[skipDirs=%s]", SKIP_DIRS);
    }
}
