package com.p2p.transfer.resume;

import com.p2p.core.util.FileUtils;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.*;
import java.nio.file.*;
import java.time.Instant;
import java.util.*;
import java.util.concurrent.ConcurrentSkipListSet;

/**
 * Manages transfer resume state persistence. Saves and loads the set of
 * completed chunk indices for interrupted transfers, allowing resumption
 * from the last checkpoint. Expired state files are periodically cleaned up
 * based on a configurable retention period. Thread-safe for concurrent
 * save/load operations.
 */
public final class TransferResumeManager {

    // --- Constants ---

    private static final Logger log = LoggerFactory.getLogger(TransferResumeManager.class);
    private static final String RESUME_EXTENSION = ".p2p-resume";

    // --- Fields ---

    private final Path resumeDir;
    private final int expiryDays;

    // --- Constructor ---

    /**
     * Creates a TransferResumeManager that stores resume state files in a
     * {@code resume/} subdirectory of the given data directory.
     *
     * @param dataDir    the base data directory
     * @param expiryDays number of days after which a resume state is
     *                   considered expired; must be non-negative
     * @throws NullPointerException     if dataDir is null
     * @throws IllegalArgumentException if expiryDays is negative
     */
    public TransferResumeManager(Path dataDir, int expiryDays) {
        Objects.requireNonNull(dataDir, "dataDir must not be null");
        if (expiryDays < 0) {
            throw new IllegalArgumentException("expiryDays must be non-negative: " + expiryDays);
        }
        this.resumeDir = dataDir.resolve("resume");
        this.expiryDays = expiryDays;
        try {
            Files.createDirectories(resumeDir);
        } catch (IOException e) {
            log.warn("Failed to create resume directory: {}", e.getMessage());
        }
    }

    // --- Public API ---

    /**
     * Saves the current transfer state to a properties file, including the
     * set of completed chunks serialised as a comma-separated list.
     *
     * @param sessionId       the transfer session identifier
     * @param fileHash        SHA-256 hash of the complete file; may be null
     * @param fileSize        total file size in bytes
     * @param totalChunks     total number of chunks in the transfer
     * @param completedChunks set of chunk indices that have been completed
     * @param targetPath      the target file path on disk
     * @throws IOException              if the state file cannot be written
     * @throws NullPointerException     if sessionId, completedChunks, or
     *                                  targetPath is null
     * @throws IllegalArgumentException if fileSize or totalChunks is negative
     */
    public void saveState(String sessionId, String fileHash, long fileSize,
                           long totalChunks, Set<Long> completedChunks,
                           Path targetPath) throws IOException {
        Objects.requireNonNull(sessionId, "sessionId must not be null");
        Objects.requireNonNull(completedChunks, "completedChunks must not be null");
        Objects.requireNonNull(targetPath, "targetPath must not be null");
        if (fileSize < 0) {
            throw new IllegalArgumentException("fileSize must be non-negative: " + fileSize);
        }
        if (totalChunks < 0) {
            throw new IllegalArgumentException("totalChunks must be non-negative: " + totalChunks);
        }

        Path resumeFile = getResumeFile(sessionId);

        Properties props = new Properties();
        props.setProperty("sessionId", sessionId);
        props.setProperty("fileHash", fileHash != null ? fileHash : "");
        props.setProperty("fileSize", String.valueOf(fileSize));
        props.setProperty("totalChunks", String.valueOf(totalChunks));
        props.setProperty("targetPath", targetPath.toString());
        props.setProperty("timestamp", Instant.now().toString());
        props.setProperty("completedChunks", setToString(completedChunks));

        try (OutputStream os = Files.newOutputStream(resumeFile)) {
            props.store(os, "P2P Transfer Resume State");
        }

        log.debug("Saved resume state for session {}: {}/{} chunks",
                sessionId, completedChunks.size(), totalChunks);
    }

    /**
     * Loads a previously saved resume state for the given session.
     *
     * @param sessionId the transfer session identifier
     * @return the ResumeState if found, or null if no state file exists or it
     *         cannot be parsed
     * @throws NullPointerException if sessionId is null
     */
    public ResumeState loadState(String sessionId) {
        Objects.requireNonNull(sessionId, "sessionId must not be null");

        Path resumeFile = getResumeFile(sessionId);
        if (!Files.exists(resumeFile)) {
            return null;
        }

        try (InputStream is = Files.newInputStream(resumeFile)) {
            Properties props = new Properties();
            props.load(is);

            return new ResumeState(
                    props.getProperty("sessionId"),
                    props.getProperty("fileHash"),
                    Long.parseLong(props.getProperty("fileSize")),
                    Long.parseLong(props.getProperty("totalChunks")),
                    stringToSet(props.getProperty("completedChunks")),
                    Path.of(props.getProperty("targetPath")),
                    Instant.parse(props.getProperty("timestamp"))
            );
        } catch (Exception e) {
            log.warn("Failed to load resume state for {}: {}", sessionId, e.getMessage());
            return null;
        }
    }

    /**
     * Removes the resume state file for the given session.
     *
     * @param sessionId the transfer session identifier
     * @throws NullPointerException if sessionId is null
     */
    public void removeState(String sessionId) {
        Objects.requireNonNull(sessionId, "sessionId must not be null");
        FileUtils.safeDelete(getResumeFile(sessionId));
    }

    /**
     * Scans the resume directory and deletes state files that have not been
     * modified within the configured retention period. Also removes the
     * associated target file to reclaim disk space.
     *
     * @return the number of expired state files cleaned up
     */
    public int cleanupExpired() {
        int cleaned = 0;
        try (DirectoryStream<Path> stream = Files.newDirectoryStream(resumeDir, "*" + RESUME_EXTENSION)) {
            Instant cutoff = Instant.now().minusSeconds(expiryDays * 86400L);

            for (Path file : stream) {
                try {
                    Instant modified = Files.getLastModifiedTime(file).toInstant();
                    if (modified.isBefore(cutoff)) {
                        ResumeState state = loadState(
                                file.getFileName().toString().replace(RESUME_EXTENSION, ""));
                        if (state != null) {
                            FileUtils.safeDelete(state.targetPath());
                        }
                        Files.deleteIfExists(file);
                        cleaned++;
                    }
                } catch (IOException e) {
                    log.debug("Error cleaning resume file {}: {}", file, e.getMessage());
                }
            }
        } catch (IOException e) {
            log.warn("Error scanning resume directory: {}", e.getMessage());
        }

        if (cleaned > 0) {
            log.info("Cleaned up {} expired resume files", cleaned);
        }
        return cleaned;
    }

    // --- Internal ---

    private Path getResumeFile(String sessionId) {
        return resumeDir.resolve(sessionId + RESUME_EXTENSION);
    }

    private static String setToString(Set<Long> set) {
        StringBuilder sb = new StringBuilder();
        Iterator<Long> iter = new TreeSet<>(set).iterator();
        while (iter.hasNext()) {
            sb.append(iter.next());
            if (iter.hasNext()) {
                sb.append(',');
            }
        }
        return sb.toString();
    }

    private static Set<Long> stringToSet(String str) {
        Set<Long> set = new ConcurrentSkipListSet<>();
        if (str == null || str.isEmpty()) {
            return set;
        }
        for (String s : str.split(",")) {
            try {
                set.add(Long.parseLong(s.trim()));
            } catch (NumberFormatException ignored) {
                // skip malformed entries
            }
        }
        return set;
    }

    // --- Records ---

    /**
     * Immutable snapshot of a transfer's resume state.
     */
    public record ResumeState(
            String sessionId,
            String fileHash,
            long fileSize,
            long totalChunks,
            Set<Long> completedChunks,
            Path targetPath,
            Instant timestamp
    ) {
        /**
         * Returns the number of completed chunks.
         *
         * @return completed chunk count
         */
        public long completedCount() {
            return completedChunks.size();
        }

        /**
         * Returns the number of remaining (uncompleted) chunks.
         *
         * @return remaining chunk count
         */
        public long remainingCount() {
            return totalChunks - completedCount();
        }

        @Override
        public String toString() {
            return String.format("ResumeState[sessionId=%s, chunks=%d/%d, timestamp=%s]",
                    sessionId, completedChunks.size(), totalChunks, timestamp);
        }
    }
}
