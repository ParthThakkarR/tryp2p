package com.p2p.security.authlog;

import com.p2p.core.model.PeerId;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.net.InetAddress;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.time.Instant;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.ConcurrentHashMap;

/**
 * Records and tracks peer authentication events for security monitoring.
 * Maintains an in-memory sliding window of recent events plus a persistent
 * audit log. Automatically detects brute-force patterns and provides
 * auto-block recommendations based on failure thresholds.
 */
public final class PeerAuthenticationLog {

    // --- Constants ---

    private static final Logger log = LoggerFactory.getLogger(PeerAuthenticationLog.class);
    private static final int ALERT_THRESHOLD = 3;
    private static final long ALERT_WINDOW_MS = 5 * 60 * 1000;
    private static final int MAX_RECENT_EVENTS = 1000;
    private static final String AUDIT_FILE_NAME = "handshake-audit.log";
    private static final String LOG_DIR_NAME = "auth-log";

    // --- Fields ---

    private final Path logDir;
    private final Map<PeerId, List<Instant>> failureTracker = new ConcurrentHashMap<>();
    private final List<AuthEvent> recentEvents = Collections.synchronizedList(new ArrayList<>());
    private final Path auditFile;

    // --- Constructor ---

    /**
     * Constructs an authentication log backed by the given data directory.
     *
     * @param dataDir the root data directory for persistent audit files
     */
    public PeerAuthenticationLog(Path dataDir) {
        Objects.requireNonNull(dataDir, "dataDir must not be null");
        this.logDir = dataDir.resolve(LOG_DIR_NAME);
        this.auditFile = logDir.resolve(AUDIT_FILE_NAME);
        try {
            Files.createDirectories(logDir);
        } catch (IOException e) {
            log.warn("Failed to create auth log directory: {}", e.getMessage());
        }
    }

    // --- Public API ---

    /**
     * Records an authentication attempt (success or failure) for a peer.
     * Failed attempts are tracked for brute-force detection and alerting.
     *
     * @param peerId        the peer that attempted authentication
     * @param sourceIp      the source IP address of the attempt
     * @param success       whether the authentication succeeded
     * @param failureReason a description of the failure, may be null on success
     */
    public void logAttempt(PeerId peerId, InetAddress sourceIp, boolean success, String failureReason) {
        Objects.requireNonNull(peerId, "peerId must not be null");
        Objects.requireNonNull(sourceIp, "sourceIp must not be null");

        AuthEvent event = new AuthEvent(
                Instant.now(), peerId, sourceIp.getHostAddress(), success, failureReason);

        recentEvents.add(event);
        if (recentEvents.size() > MAX_RECENT_EVENTS) {
            recentEvents.removeFirst();
        }

        appendToFile(event);

        if (!success) {
            trackFailure(peerId);
        } else {
            failureTracker.remove(peerId);
        }

        log.info("Auth {} for peer {} from {}{}",
                success ? "SUCCESS" : "FAILURE",
                peerId.toShortString(),
                sourceIp.getHostAddress(),
                failureReason != null ? " - " + failureReason : "");
    }

    /**
     * Returns whether the given peer has exceeded the failure threshold
     * within the alert window and should be automatically blocked.
     *
     * @param peerId the peer to evaluate
     * @return true if the peer should be auto-blocked
     */
    public boolean shouldAutoBlock(PeerId peerId) {
        Objects.requireNonNull(peerId, "peerId must not be null");
        List<Instant> failures = failureTracker.getOrDefault(peerId, List.of());
        Instant now = Instant.now();
        long recentFailures = failures.stream()
                .filter(t -> t.toEpochMilli() + ALERT_WINDOW_MS >= now.toEpochMilli())
                .count();
        return recentFailures >= ALERT_THRESHOLD;
    }

    /**
     * Returns the most recent authentication events, up to the requested count.
     *
     * @param count the maximum number of events to return
     * @return an immutable list of recent auth events
     */
    public List<AuthEvent> getRecentEvents(int count) {
        synchronized (recentEvents) {
            int start = Math.max(0, recentEvents.size() - count);
            return List.copyOf(recentEvents.subList(start, recentEvents.size()));
        }
    }

    /**
     * Returns all stored authentication events for a specific peer.
     *
     * @param peerId the peer to filter by
     * @return a list of matching auth events
     */
    public List<AuthEvent> getEventsForPeer(PeerId peerId) {
        Objects.requireNonNull(peerId, "peerId must not be null");
        synchronized (recentEvents) {
            return recentEvents.stream()
                    .filter(e -> e.peerId().equals(peerId))
                    .toList();
        }
    }

    // --- Internal ---

    private void trackFailure(PeerId peerId) {
        List<Instant> failures = failureTracker.computeIfAbsent(peerId, k -> new ArrayList<>());
        Instant now = Instant.now();
        failures.removeIf(t -> t.toEpochMilli() + ALERT_WINDOW_MS < now.toEpochMilli());
        failures.add(now);

        if (failures.size() >= ALERT_THRESHOLD) {
            log.error("SECURITY ALERT: {} failed auth attempts from peer {} in {} minutes",
                    failures.size(), peerId.toShortString(), ALERT_WINDOW_MS / 60000);
        }
    }

    private void appendToFile(AuthEvent event) {
        try {
            String line = String.format("%s|%s|%s|%s|%s\n",
                    event.timestamp(), event.peerId(),
                    event.sourceIp(), event.success(), event.failureReason());
            Files.write(auditFile, List.of(line),
                    StandardOpenOption.CREATE, StandardOpenOption.APPEND);
        } catch (IOException e) {
            log.debug("Failed to write auth log: {}", e.getMessage());
        }
    }

    @Override
    public String toString() {
        return String.format("PeerAuthenticationLog[recentEvents=%d, auditFile=%s]",
                recentEvents.size(), auditFile);
    }

    // --- Value types ---

    /**
     * A record representing a peer authentication event.
     *
     * @param timestamp     when the event occurred
     * @param peerId        the peer that attempted authentication
     * @param sourceIp      the source IP address
     * @param success       whether authentication succeeded
     * @param failureReason description of the failure, null on success
     */
    public record AuthEvent(Instant timestamp, PeerId peerId, String sourceIp,
                             boolean success, String failureReason) {}
}
