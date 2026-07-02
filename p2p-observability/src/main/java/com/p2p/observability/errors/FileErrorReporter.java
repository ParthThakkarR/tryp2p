package com.p2p.observability.errors;

import com.p2p.core.model.ChunkInfo;
import com.p2p.core.model.FileMetadata;
import com.p2p.core.model.PeerId;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.file.*;
import java.time.Instant;
import java.time.ZoneId;
import java.time.format.DateTimeFormatter;
import java.util.Objects;

/**
 * File-based {@link ErrorReporter} that appends structured error lines
 * to {@code errors/error-report.log} under the configured data directory.
 */
public final class FileErrorReporter implements ErrorReporter {

    // --- Constants ---

    private static final Logger log = LoggerFactory.getLogger(FileErrorReporter.class);
    private static final DateTimeFormatter FORMATTER = DateTimeFormatter.ISO_INSTANT
            .withZone(ZoneId.of("UTC"));

    // --- Fields ---

    private final Path errorFile;

    // --- Constructor ---

    /**
     * Creates a file-based error reporter that writes under {@code dataDir/errors/}.
     *
     * @param dataDir the root data directory (must not be null)
     */
    public FileErrorReporter(Path dataDir) {
        Objects.requireNonNull(dataDir, "dataDir must not be null");
        this.errorFile = dataDir.resolve("errors").resolve("error-report.log");
        try {
            Files.createDirectories(errorFile.getParent());
        } catch (IOException e) {
            log.warn("Failed to create error log directory: {}", e.getMessage());
        }
    }

    // --- Public API ---

    /**
     * Formats and appends an error line to the error log file.
     *
     * @param context the full error context (must not be null)
     */
    @Override
    public void reportError(ErrorContext context) {
        Objects.requireNonNull(context, "error context must not be null");
        String line = formatError(context);
        try {
            Files.write(errorFile, (line + "\n").getBytes(),
                    StandardOpenOption.CREATE, StandardOpenOption.APPEND);
        } catch (IOException e) {
            log.warn("Failed to write error report: {}", e.getMessage());
        }
        log.error("Error reported: {} - {} (peer: {})", context.errorType(), context.message(), context.peerId());
    }

    // --- Private helpers ---

    private String formatError(ErrorContext ctx) {
        return String.format("[%s] [%s] peer=%s session=%s msg=%s file=%s chunk=%d",
                FORMATTER.format(ctx.timestamp()), ctx.errorType(),
                ctx.peerId() != null ? ctx.peerId().toShortString() : "?",
                ctx.sessionId() != null ? ctx.sessionId().substring(0, 8) : "?",
                ctx.message(),
                ctx.fileInfo() != null ? ctx.fileInfo().getFileName() : "?",
                ctx.chunkInfo() != null ? ctx.chunkInfo().getChunkIndex() : -1);
    }
}
