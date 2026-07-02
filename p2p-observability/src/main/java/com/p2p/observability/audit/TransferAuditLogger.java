package com.p2p.observability.audit;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.file.*;
import java.time.Instant;
import java.util.List;
import java.util.Objects;

/**
 * Logs transfer audit events to a local file with JSON-formatted entries.
 * Automatically prunes entries older than the configured retention period.
 */
public final class TransferAuditLogger implements AutoCloseable {

    // --- Constants ---

    private static final Logger log = LoggerFactory.getLogger(TransferAuditLogger.class);
    private static final long SECONDS_PER_DAY = 86400L;

    // --- Fields ---

    private final Path auditFile;
    private final int retentionDays;

    // --- Constructor ---

    /**
     * Creates an audit logger that writes entries under {@code dataDir/audit/transfer-audit.log}.
     *
     * @param dataDir       the root data directory (must not be null)
     * @param retentionDays number of days to retain entries (must be non-negative)
     */
    public TransferAuditLogger(Path dataDir, int retentionDays) {
        Objects.requireNonNull(dataDir, "dataDir must not be null");
        if (retentionDays < 0) {
            throw new IllegalArgumentException("retentionDays must be non-negative, got: " + retentionDays);
        }
        this.auditFile = dataDir.resolve("audit").resolve("transfer-audit.log");
        this.retentionDays = retentionDays;
        try {
            Files.createDirectories(auditFile.getParent());
        } catch (IOException e) {
            log.warn("Failed to create audit directory: {}", e.getMessage());
        }
        cleanupOldEntries();
    }

    // --- Public API ---

    /**
     * Logs a single audit entry synchronously (thread-safe).
     *
     * @param entry the entry to persist (must not be null)
     */
    public synchronized void logTransfer(AuditEntry entry) {
        Objects.requireNonNull(entry, "audit entry must not be null");
        try {
            String line = entry.toJson() + "\n";
            Files.write(auditFile, line.getBytes(),
                    StandardOpenOption.CREATE, StandardOpenOption.APPEND);
            log.debug("Audit entry logged: {} - {}", entry.transferId(), entry.status());
        } catch (IOException e) {
            log.warn("Failed to write audit entry: {}", e.getMessage());
        }
    }

    /**
     * Convenience method that constructs an {@link AuditEntry} from individual fields
     * and delegates to {@link #logTransfer(AuditEntry)}.
     */
    public synchronized void logTransfer(String transferId, String fileHash, String fileName,
                                          long fileSize, long durationMs,
                                          String sourcePeer, String destPeer,
                                          String status, String errorMessage) {
        logTransfer(new AuditEntry(
                Instant.now().toString(), transferId, fileHash, fileName,
                fileSize, durationMs, sourcePeer, destPeer, status, errorMessage));
    }

    /**
     * Returns the complete transfer history from the audit file.
     *
     * @return list of all audit lines (may be empty)
     * @throws IOException if the file cannot be read
     */
    public synchronized List<String> getTransferHistory() throws IOException {
        if (Files.exists(auditFile)) {
            return Files.readAllLines(auditFile);
        }
        return List.of();
    }

    @Override
    public void close() {
        // nothing to close — file handle is released per write
    }

    // --- Private helpers ---

    private void cleanupOldEntries() {
        try {
            if (Files.exists(auditFile)) {
                long cutoffEpoch = Instant.now()
                        .minusSeconds(retentionDays * SECONDS_PER_DAY).toEpochMilli();
                List<String> lines = Files.readAllLines(auditFile);
                List<String> fresh = lines.stream()
                        .filter(l -> parseTimestamp(l) > cutoffEpoch)
                        .toList();
                if (fresh.size() < lines.size()) {
                    Files.write(auditFile, fresh);
                    log.info("Cleaned {} expired audit entries", lines.size() - fresh.size());
                }
            }
        } catch (IOException e) {
            log.warn("Failed to clean audit entries: {}", e.getMessage());
        }
    }

    private long parseTimestamp(String line) {
        try {
            if (line.contains("\"timestamp\":")) {
                String ts = line.replaceAll(".*\"timestamp\":\"([^\"]+)\".*", "$1");
                return Instant.parse(ts).toEpochMilli();
            }
        } catch (Exception ignored) {
            // non-critical parse failure
        }
        return 0;
    }

    // --- Audit entry record ---

    /**
     * Immutable record representing a single transfer audit entry.
     */
    public record AuditEntry(
            String timestamp, String transferId, String fileHash,
            String fileName, long fileSize, long durationMs,
            String sourcePeer, String destPeer,
            String status, String errorMessage
    ) {
        /**
         * Serializes this entry to a single-line JSON string.
         *
         * @return JSON representation
         */
        public String toJson() {
            return String.format(
                    "{\"timestamp\":\"%s\",\"transferId\":\"%s\",\"fileHash\":\"%s\",\"fileName\":\"%s\",\"fileSize\":%d,\"durationMs\":%d,\"sourcePeer\":\"%s\",\"destPeer\":\"%s\",\"status\":\"%s\",\"errorMessage\":\"%s\"}",
                    timestamp, transferId, fileHash, fileName, fileSize, durationMs,
                    sourcePeer, destPeer, status,
                    errorMessage != null ? errorMessage.replace("\"", "\\\"") : "");
        }
    }
}
