package com.p2p.security.ratelimit;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Duration;
import java.time.Instant;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.locks.ReentrantLock;

/**
 * Token-bucket rate limiter that tracks request frequency per peer.
 * Peers that exceed the configured threshold are automatically blocked
 * for a configurable duration. The block list is persisted to disk.
 */
public final class RateLimiter {

    // --- Constants ---

    private static final Logger log = LoggerFactory.getLogger(RateLimiter.class);
    private static final long DEFAULT_BLOCK_DURATION_MINUTES = 15;

    // --- Fields ---

    private final int maxRequestsPerMinute;
    private final Duration blockDuration;
    private final Map<String, TokenBucket> buckets = new ConcurrentHashMap<>();
    private final Set<String> blockedPeers = ConcurrentHashMap.newKeySet();
    private final ReentrantLock blockLock = new ReentrantLock();
    private final Path blockListPath;

    // --- Constructor ---

    /**
     * Constructs a rate limiter with the given limits and persistence directory.
     *
     * @param maxRequestsPerMinute maximum requests allowed per minute per peer (must be positive)
     * @param blockDuration        duration a peer remains blocked after exceeding the limit
     * @param dataDir              directory for persistent block list storage
     */
    public RateLimiter(int maxRequestsPerMinute, Duration blockDuration, Path dataDir) {
        if (maxRequestsPerMinute <= 0) {
            throw new IllegalArgumentException(
                    "maxRequestsPerMinute must be positive: " + maxRequestsPerMinute);
        }
        this.maxRequestsPerMinute = maxRequestsPerMinute;
        this.blockDuration = Objects.requireNonNull(blockDuration, "blockDuration must not be null");
        this.blockListPath = Objects.requireNonNull(dataDir, "dataDir must not be null")
                .resolve("blocked-peers.txt");
        loadBlockList();
    }

    // --- Public API ---

    /**
     * Checks whether a request from the given peer is allowed.
     * If the peer exceeds the rate limit, it is automatically blocked.
     *
     * @param peerId the peer identifier
     * @return true if the request is allowed, false if blocked or rate-limited
     */
    public boolean allowRequest(String peerId) {
        Objects.requireNonNull(peerId, "peerId must not be null");
        if (isBlocked(peerId)) {
            log.debug("Request blocked for peer: {}", peerId);
            return false;
        }
        TokenBucket bucket = buckets.computeIfAbsent(peerId,
                k -> new TokenBucket(maxRequestsPerMinute, Duration.ofMinutes(1)));
        boolean allowed = bucket.tryConsume();
        if (!allowed) {
            log.warn("Rate limit exceeded for peer: {}", peerId);
            blockPeer(peerId);
        }
        return allowed;
    }

    /**
     * Returns whether the given peer is currently blocked.
     *
     * @param peerId the peer identifier
     * @return true if the peer is blocked
     */
    public boolean isBlocked(String peerId) {
        Objects.requireNonNull(peerId, "peerId must not be null");
        cleanupBlocked();
        return blockedPeers.contains(peerId);
    }

    /**
     * Manually blocks a peer, adding it to the persistent block list.
     *
     * @param peerId the peer to block
     */
    public void blockPeer(String peerId) {
        Objects.requireNonNull(peerId, "peerId must not be null");
        blockLock.lock();
        try {
            blockedPeers.add(peerId);
            saveBlockList();
            log.info("Blocked peer: {} (duration: {})", peerId, blockDuration);
        } finally {
            blockLock.unlock();
        }
    }

    /**
     * Manually unblocks a peer, removing it from the persistent block list.
     *
     * @param peerId the peer to unblock
     */
    public void unblockPeer(String peerId) {
        Objects.requireNonNull(peerId, "peerId must not be null");
        blockLock.lock();
        try {
            blockedPeers.remove(peerId);
            saveBlockList();
            log.info("Unblocked peer: {}", peerId);
        } finally {
            blockLock.unlock();
        }
    }

    /**
     * Returns the number of currently blocked peers (after expiring stale entries).
     *
     * @return the count of blocked peers
     */
    public int getBlockedCount() {
        cleanupBlocked();
        return blockedPeers.size();
    }

    // --- Internal ---

    private void cleanupBlocked() {
        Instant now = Instant.now();
        blockedPeers.removeIf(peer -> {
            TokenBucket bucket = buckets.get(peer);
            return bucket != null && bucket.isExpired(now);
        });
    }

    private void loadBlockList() {
        try {
            if (Files.exists(blockListPath)) {
                List<String> lines = Files.readAllLines(blockListPath);
                blockedPeers.addAll(lines);
                log.info("Loaded {} blocked peers from disk", lines.size());
            }
        } catch (IOException e) {
            log.warn("Failed to load block list: {}", e.getMessage());
        }
    }

    private void saveBlockList() {
        try {
            Files.write(blockListPath, new ArrayList<>(blockedPeers));
        } catch (IOException e) {
            log.warn("Failed to save block list: {}", e.getMessage());
        }
    }

    @Override
    public String toString() {
        return String.format("RateLimiter[maxRequestsPerMinute=%s, blocked=%d]",
                maxRequestsPerMinute, blockedPeers.size());
    }

    /**
     * Per-peer token bucket used for rate-limit tracking.
     * Tokens are refilled continuously over a one-minute window.
     */
    private static final class TokenBucket {
        private final int maxTokens;
        private final Duration refillInterval;
        private double tokens;
        private Instant lastRefill;
        private Instant blockedAt;

        TokenBucket(int maxTokens, Duration refillInterval) {
            this.maxTokens = maxTokens;
            this.refillInterval = refillInterval;
            this.tokens = maxTokens;
            this.lastRefill = Instant.now();
            this.blockedAt = null;
        }

        synchronized boolean tryConsume() {
            refill();
            if (tokens >= 1) {
                tokens--;
                return true;
            }
            blockedAt = Instant.now();
            return false;
        }

        boolean isExpired(Instant now) {
            if (blockedAt == null) return false;
            return Duration.between(blockedAt, now)
                    .compareTo(Duration.ofMinutes(DEFAULT_BLOCK_DURATION_MINUTES)) >= 0;
        }

        private void refill() {
            Instant now = Instant.now();
            long elapsed = Duration.between(lastRefill, now).toMillis();
            double refillAmount = (double) elapsed / refillInterval.toMillis() * maxTokens;
            tokens = Math.min(maxTokens, tokens + refillAmount);
            lastRefill = now;
        }
    }
}
