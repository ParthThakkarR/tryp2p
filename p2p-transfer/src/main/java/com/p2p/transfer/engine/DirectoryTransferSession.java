package com.p2p.transfer.engine;

import com.p2p.core.model.*;
import com.p2p.core.util.FileUtils;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.*;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicLong;

/**
 * Manages a directory-level transfer as a session comprising multiple file
 * transfers. Tracks completion and failure counts per file, maintains transfer
 * state, and supports target/source path resolution. Thread-safe for
 * concurrent file completion recording via atomic fields and a concurrent map.
 */
public final class DirectoryTransferSession {

    // --- Constants ---

    private static final Logger log = LoggerFactory.getLogger(DirectoryTransferSession.class);

    // --- Fields ---

    private final String sessionId;
    private final Path sourceDirectory;
    private final Path targetDirectory;
    private final List<FileMetadata> entries;
    private final AtomicInteger filesCompleted = new AtomicInteger(0);
    private final AtomicInteger filesFailed = new AtomicInteger(0);
    private final AtomicLong totalBytesTransferred = new AtomicLong(0);
    private final Map<Integer, TransferSession> fileSessions = new ConcurrentHashMap<>();
    private final DirectoryScanner scanner = new DirectoryScanner();
    private volatile TransferState state = TransferState.PENDING;

    // --- Constructor ---

    /**
     * Creates a new directory transfer session. Scans the source directory to
     * build the file manifest and creates the target directory tree.
     *
     * @param sessionId       unique identifier for this session
     * @param sourceDirectory the source directory to transfer
     * @param targetDirectory the destination directory on the receiving side
     * @throws IOException              if scanning or directory creation fails
     * @throws NullPointerException     if any argument is null
     * @throws IllegalArgumentException if sourceDirectory is not a directory
     */
    public DirectoryTransferSession(String sessionId, Path sourceDirectory, Path targetDirectory) throws IOException {
        this.sessionId = Objects.requireNonNull(sessionId, "sessionId must not be null");
        this.sourceDirectory = Objects.requireNonNull(sourceDirectory, "sourceDirectory must not be null");
        this.targetDirectory = Objects.requireNonNull(targetDirectory, "targetDirectory must not be null");
        if (!Files.isDirectory(sourceDirectory)) {
            throw new IllegalArgumentException("Not a directory: " + sourceDirectory);
        }

        this.entries = scanner.scan(sourceDirectory, 1024 * 1024);
        Files.createDirectories(targetDirectory);

        log.info("Directory transfer session created: {} ({} files, {} directories)",
                sessionId, scanner.countFiles(entries), scanner.countDirectories(entries));
    }

    // --- Public Accessors ---

    /**
     * Returns the session identifier.
     *
     * @return session ID
     */
    public String getSessionId() {
        return sessionId;
    }

    /**
     * Returns the source directory path.
     *
     * @return source directory
     */
    public Path getSourceDirectory() {
        return sourceDirectory;
    }

    /**
     * Returns the target directory path.
     *
     * @return target directory
     */
    public Path getTargetDirectory() {
        return targetDirectory;
    }

    /**
     * Returns an immutable copy of the scanned file entries.
     *
     * @return list of FileMetadata entries
     */
    public List<FileMetadata> getEntries() {
        return List.copyOf(entries);
    }

    /**
     * Returns the current transfer state.
     *
     * @return current state
     */
    public TransferState getState() {
        return state;
    }

    /**
     * Returns the number of files completed so far.
     *
     * @return completed file count
     */
    public int getFilesCompleted() {
        return filesCompleted.get();
    }

    /**
     * Returns the number of files that have failed.
     *
     * @return failed file count
     */
    public int getFilesFailed() {
        return filesFailed.get();
    }

    /**
     * Returns the total number of bytes transferred across all files.
     *
     * @return total bytes
     */
    public long getTotalBytesTransferred() {
        return totalBytesTransferred.get();
    }

    // --- State Management ---

    /**
     * Sets the current transfer state.
     *
     * @param newState the new state
     * @throws NullPointerException if newState is null
     */
    public void setState(TransferState newState) {
        this.state = Objects.requireNonNull(newState, "newState must not be null");
    }

    /**
     * Records a successful file transfer, incrementing the completed count and
     * adding the bytes transferred.
     *
     * @param relativePath     the relative path of the completed file (for logging)
     * @param bytesTransferred number of bytes transferred in this file
     * @throws NullPointerException if relativePath is null
     */
    public void recordFileCompleted(String relativePath, long bytesTransferred) {
        Objects.requireNonNull(relativePath, "relativePath must not be null");
        filesCompleted.incrementAndGet();
        totalBytesTransferred.addAndGet(bytesTransferred);
        log.debug("File completed: {} ({} total)", relativePath, filesCompleted.get());
    }

    /**
     * Records a failed file transfer.
     *
     * @param relativePath the relative path of the failed file (for logging)
     * @throws NullPointerException if relativePath is null
     */
    public void recordFileFailed(String relativePath) {
        Objects.requireNonNull(relativePath, "relativePath must not be null");
        filesFailed.incrementAndGet();
        log.warn("File failed: {} ({} total)", relativePath, filesFailed.get());
    }

    /**
     * Registers a sub-session for an individual file transfer.
     *
     * @param index   the index of the file within the entries list
     * @param session the transfer session for that file
     */
    public void registerFileSession(int index, TransferSession session) {
        if (session == null) {
            throw new NullPointerException("session must not be null");
        }
        fileSessions.put(index, session);
    }

    /**
     * Returns the transfer session for a specific file index, if registered.
     *
     * @param index the file index
     * @return an Optional containing the session, or empty if not registered
     */
    public Optional<TransferSession> getFileSession(int index) {
        return Optional.ofNullable(fileSessions.get(index));
    }

    /**
     * Returns an unmodifiable view of all file sub-sessions.
     *
     * @return map of file index to TransferSession
     */
    public Map<Integer, TransferSession> getAllFileSessions() {
        return Collections.unmodifiableMap(fileSessions);
    }

    // --- Derived Queries ---

    /**
     * Returns the overall progress as a percentage of completed files.
     *
     * @return progress percentage (0.0 – 100.0)
     */
    public double getProgressPercentage() {
        int totalFiles = scanner.countFiles(entries);
        if (totalFiles == 0) {
            return 100.0;
        }
        return (double) filesCompleted.get() / totalFiles * 100.0;
    }

    /**
     * Returns the total size of all files in the directory.
     *
     * @return total file size in bytes
     */
    public long getTotalSize() {
        return scanner.calculateTotalSize(entries);
    }

    /**
     * Returns a human-readable formatted total size string.
     *
     * @return e.g. "1.5 MB", "340 B"
     */
    public String getFormattedTotalSize() {
        return formatSize(getTotalSize());
    }

    // --- Path Resolution ---

    /**
     * Resolves a file metadata entry to its absolute target path.
     *
     * @param entry the file metadata
     * @return the resolved target path
     * @throws NullPointerException if entry is null
     */
    public Path resolveTargetPath(FileMetadata entry) {
        Objects.requireNonNull(entry, "entry must not be null");
        return targetDirectory.resolve(entry.getRelativePath());
    }

    /**
     * Resolves a file metadata entry to its absolute source path.
     *
     * @param entry the file metadata
     * @return the resolved source path
     * @throws NullPointerException if entry is null
     */
    public Path resolveSourcePath(FileMetadata entry) {
        Objects.requireNonNull(entry, "entry must not be null");
        return sourceDirectory.resolve(entry.getRelativePath());
    }

    /**
     * Creates all target subdirectories for the scanned entries.
     *
     * @throws IOException if any directory cannot be created
     */
    public void createDirectories() throws IOException {
        for (FileMetadata entry : entries) {
            if (entry.isDirectory()) {
                Path dir = targetDirectory.resolve(entry.getRelativePath());
                Files.createDirectories(dir);
                log.debug("Created directory: {}", dir);
            }
        }
    }

    // --- Internal ---

    private static String formatSize(long bytes) {
        if (bytes < 1024) {
            return bytes + " B";
        }
        if (bytes < 1024 * 1024) {
            return String.format("%.1f KB", bytes / 1024.0);
        }
        if (bytes < 1024 * 1024 * 1024) {
            return String.format("%.1f MB", bytes / (1024.0 * 1024));
        }
        return String.format("%.2f GB", bytes / (1024.0 * 1024 * 1024));
    }

    // --- Object ---

    @Override
    public String toString() {
        return String.format("DirectoryTransferSession[id=%s, source=%s, %d/%d files, %.1f%%]",
                sessionId.length() > 8 ? sessionId.substring(0, 8) : sessionId,
                sourceDirectory.getFileName(),
                filesCompleted.get(),
                scanner.countFiles(entries),
                getProgressPercentage());
    }
}
