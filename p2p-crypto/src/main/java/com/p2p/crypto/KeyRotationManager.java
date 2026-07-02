package com.p2p.crypto;

import com.p2p.core.exception.CryptoException;
import com.p2p.core.exception.ErrorCode;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.security.MessageDigest;
import java.security.PrivateKey;
import java.security.PublicKey;
import java.security.SecureRandom;
import java.security.spec.ECGenParameterSpec;
import java.time.Duration;
import java.time.Instant;
import java.util.ArrayList;
import java.util.List;
import java.util.Objects;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.locks.ReentrantReadWriteLock;

/**
 * Manages periodic ECDSA key pair rotation for peer identity.
 * Maintains a history of the current and previous key generation with a fallback
 * window ({@value #WINDOW_HOURS} hour) during which the previous key is still
 * considered valid. This allows gradual key propagation across the network
 * without dropping in-flight connections.
 * <p>
 * Thread-safe. Implements {@link AutoCloseable} for clean shutdown of the
 * background scheduler thread.
 */
public final class KeyRotationManager implements AutoCloseable {

    // --- Constants ---

    private static final Logger log = LoggerFactory.getLogger(KeyRotationManager.class);
    private static final long WINDOW_HOURS = 1;
    private static final Duration ROTATION_WINDOW = Duration.ofHours(WINDOW_HOURS);
    private static final int MAX_HISTORY = 2;
    private static final String KEY_ALGORITHM = "EC";
    private static final String KEY_CURVE = "secp256r1";
    private static final long SCHEDULER_SHUTDOWN_TIMEOUT_SECONDS = 5;

    // --- Fields ---

    private final Duration rotationInterval;
    private final ScheduledExecutorService scheduler;
    private final ReentrantReadWriteLock lock = new ReentrantReadWriteLock();
    private final List<KeyGeneration> keyHistory = new ArrayList<>();
    private final AtomicInteger generationCounter = new AtomicInteger();
    private volatile boolean running;

    // --- Constructor ---

    /**
     * Creates a key rotation manager with the specified rotation interval.
     *
     * @param rotationInterval how often to generate a new key pair
     * @throws IllegalArgumentException if rotationInterval is null or non-positive
     */
    public KeyRotationManager(Duration rotationInterval) {
        if (rotationInterval == null) {
            throw new IllegalArgumentException("Rotation interval must not be null");
        }
        if (rotationInterval.isNegative() || rotationInterval.isZero()) {
            throw new IllegalArgumentException(
                    "Rotation interval must be positive, got: " + rotationInterval);
        }
        this.rotationInterval = rotationInterval;
        this.scheduler = Executors.newSingleThreadScheduledExecutor(r -> {
            Thread t = new Thread(r, "key-rotation");
            t.setDaemon(true);
            return t;
        });
    }

    // --- Lifecycle ---

    /**
     * Starts the key rotation manager. Generates the initial key pair immediately
     * and schedules periodic rotation at the configured interval.
     */
    public void start() {
        generateNewKey();
        running = true;
        scheduler.scheduleAtFixedRate(this::rotateIfNeeded,
                rotationInterval.toMinutes(), rotationInterval.toMinutes(), TimeUnit.MINUTES);
        log.info("Key rotation started, interval: {}", rotationInterval);
    }

    /**
     * Stops the key rotation manager. Existing keys remain valid; no new keys
     * are generated. Call {@link #close()} for full resource cleanup including
     * scheduler shutdown.
     */
    public void stop() {
        running = false;
    }

    /**
     * Stops rotation and shuts down the background scheduler thread.
     * Waits up to {@value #SCHEDULER_SHUTDOWN_TIMEOUT_SECONDS} seconds for
     * pending tasks to complete.
     */
    @Override
    public void close() {
        stop();
        scheduler.shutdownNow();
        try {
            if (!scheduler.awaitTermination(SCHEDULER_SHUTDOWN_TIMEOUT_SECONDS, TimeUnit.SECONDS)) {
                log.warn("Key rotation scheduler did not terminate in time");
            }
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        }
    }

    // --- Key Lookup ---

    /**
     * Returns the most recently generated key pair (the current generation).
     *
     * @return the current {@link KeyGeneration}, or {@code null} if no keys exist
     */
    public KeyGeneration getCurrentKey() {
        lock.readLock().lock();
        try {
            if (keyHistory.isEmpty()) return null;
            return keyHistory.get(keyHistory.size() - 1);
        } finally {
            lock.readLock().unlock();
        }
    }

    /**
     * Checks whether a given public key is valid for the current time.
     * The key is valid if it matches the current key generation, or if it is
     * the previous generation and is still within the fallback window
     * ({@value #WINDOW_HOURS} hour after creation).
     *
     * @param publicKeyBytes the X.509-encoded public key to validate
     * @return {@code true} if the key is recognised and within its validity window
     * @throws CryptoException if publicKeyBytes is null
     */
    public boolean isKeyValid(byte[] publicKeyBytes) throws CryptoException {
        if (publicKeyBytes == null) {
            throw new CryptoException(ErrorCode.HANDSHAKE_FAILED,
                    "Public key bytes must not be null");
        }
        lock.readLock().lock();
        try {
            Instant now = Instant.now();
            for (int i = keyHistory.size() - 1; i >= 0; i--) {
                KeyGeneration kg = keyHistory.get(i);
                if (MessageDigest.isEqual(kg.getPublicKey().getEncoded(), publicKeyBytes)) {
                    if (i == keyHistory.size() - 1) return true;
                    Duration age = Duration.between(kg.getCreatedAt(), now);
                    if (age.compareTo(ROTATION_WINDOW) <= 0) return true;
                }
            }
            return false;
        } finally {
            lock.readLock().unlock();
        }
    }

    /**
     * Finds a key pair by its encoded public key bytes.
     * Searches the entire key history including expired generations.
     *
     * @param publicKeyBytes the X.509-encoded public key to look up
     * @return the matching {@link KeyPair}, or {@code null} if not found
     * @throws CryptoException if publicKeyBytes is null
     */
    public KeyPair findKeyPairByPublicKey(byte[] publicKeyBytes) throws CryptoException {
        if (publicKeyBytes == null) {
            throw new CryptoException(ErrorCode.HANDSHAKE_FAILED,
                    "Public key bytes must not be null");
        }
        lock.readLock().lock();
        try {
            for (KeyGeneration kg : keyHistory) {
                if (MessageDigest.isEqual(kg.getPublicKey().getEncoded(), publicKeyBytes)) {
                    return new KeyPair(kg.getPublicKey(), kg.getPrivateKey());
                }
            }
            return null;
        } finally {
            lock.readLock().unlock();
        }
    }

    // --- Internal ---

    private void rotateIfNeeded() {
        if (!running) return;
        KeyGeneration current = getCurrentKey();
        if (current == null) return;
        Duration age = Duration.between(current.getCreatedAt(), Instant.now());
        if (age.compareTo(rotationInterval) >= 0) {
            generateNewKey();
            log.info("Key rotated to generation {}", getCurrentKey().getGeneration());
        }
    }

    private void generateNewKey() {
        try {
            KeyPairGenerator kpg = KeyPairGenerator.getInstance(KEY_ALGORITHM);
            kpg.initialize(new ECGenParameterSpec(KEY_CURVE), new SecureRandom());
            KeyPair keyPair = kpg.generateKeyPair();
            int gen = generationCounter.incrementAndGet();
            KeyGeneration kg = new KeyGeneration(gen, keyPair.getPublic(), keyPair.getPrivate(), Instant.now());

            lock.writeLock().lock();
            try {
                keyHistory.add(kg);
                while (keyHistory.size() > MAX_HISTORY) {
                    keyHistory.removeFirst();
                }
            } finally {
                lock.writeLock().unlock();
            }
            log.info("Generated new key pair (generation {})", gen);
        } catch (Exception e) {
            log.error("Failed to generate key pair: {}", e.getMessage(), e);
        }
    }

    // --- Inner types ---

    /**
     * A single generation of an ECDSA key pair with its creation timestamp.
     * Immutable after construction.
     */
    public static final class KeyGeneration {

        private final int generation;
        private final PublicKey publicKey;
        private final PrivateKey privateKey;
        private final Instant createdAt;

        /**
         * Creates a new key generation record.
         *
         * @param generation the generation number (monotonically increasing)
         * @param publicKey  the ECDSA public key
         * @param privateKey the ECDSA private key
         * @param createdAt  the instant at which the key pair was generated
         */
        public KeyGeneration(int generation, PublicKey publicKey, PrivateKey privateKey, Instant createdAt) {
            if (publicKey == null) {
                throw new IllegalArgumentException("Public key must not be null");
            }
            if (privateKey == null) {
                throw new IllegalArgumentException("Private key must not be null");
            }
            if (createdAt == null) {
                throw new IllegalArgumentException("Created timestamp must not be null");
            }
            this.generation = generation;
            this.publicKey = publicKey;
            this.privateKey = privateKey;
            this.createdAt = createdAt;
        }

        // --- Getters ---

        public int getGeneration() { return generation; }
        public PublicKey getPublicKey() { return publicKey; }
        public PrivateKey getPrivateKey() { return privateKey; }
        public Instant getCreatedAt() { return createdAt; }

        @Override
        public String toString() {
            return String.format("KeyGeneration[gen=%d, algorithm=%s, createdAt=%s]",
                    generation, publicKey.getAlgorithm(), createdAt);
        }
    }

    @Override
    public String toString() {
        return String.format("KeyRotationManager[interval=%s, running=%s, generations=%d]",
                rotationInterval, running, keyHistory.size());
    }
}
