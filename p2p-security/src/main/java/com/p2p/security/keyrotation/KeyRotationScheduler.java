package com.p2p.security.keyrotation;

import com.p2p.core.config.AppConfig;
import com.p2p.crypto.KeyRotationManager;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.time.Duration;
import java.time.Instant;
import java.util.Objects;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;

/**
 * Schedules periodic key rotation checks using a {@link KeyRotationManager}.
 * Runs a background daemon thread that logs the current key age once per hour.
 * Implements {@link AutoCloseable} for clean shutdown.
 */
public final class KeyRotationScheduler implements AutoCloseable {

    // --- Constants ---

    private static final Logger log = LoggerFactory.getLogger(KeyRotationScheduler.class);
    private static final long CHECK_INTERVAL_MINUTES = 60;
    private static final long INITIAL_DELAY_MINUTES = 1;
    private static final long SHUTDOWN_TIMEOUT_SECONDS = 5;
    private static final String THREAD_NAME = "key-rotation-scheduler";

    // --- Fields ---

    private final KeyRotationManager keyRotationManager;
    private final ScheduledExecutorService scheduler;

    // --- Constructor ---

    /**
     * Constructs a key rotation scheduler from the application configuration.
     *
     * @param config the application configuration providing the rotation interval
     */
    public KeyRotationScheduler(AppConfig config) {
        Objects.requireNonNull(config, "config must not be null");
        this.keyRotationManager = new KeyRotationManager(config.getKeyRotationInterval());
        this.scheduler = Executors.newSingleThreadScheduledExecutor(r -> {
            Thread t = new Thread(r, THREAD_NAME);
            t.setDaemon(true);
            return t;
        });
    }

    // --- Public API ---

    /**
     * Starts the key rotation manager and schedules periodic rotation checks.
     */
    public void start() {
        keyRotationManager.start();
        scheduler.scheduleAtFixedRate(this::checkRotation,
                INITIAL_DELAY_MINUTES, CHECK_INTERVAL_MINUTES, TimeUnit.MINUTES);
        log.info("Key rotation scheduler started");
    }

    /**
     * Stops the scheduler and shuts down the key rotation manager.
     * Does not wait for running tasks to complete.
     */
    public void stop() {
        scheduler.shutdownNow();
        keyRotationManager.stop();
    }

    /**
     * Returns the underlying key rotation manager for direct access.
     *
     * @return the key rotation manager
     */
    public KeyRotationManager getKeyRotationManager() {
        return keyRotationManager;
    }

    /**
     * Gracefully shuts down the scheduler, waiting for task termination,
     * then closes the key rotation manager.
     */
    @Override
    public void close() {
        stop();
        try {
            scheduler.awaitTermination(SHUTDOWN_TIMEOUT_SECONDS, TimeUnit.SECONDS);
        } catch (InterruptedException ignored) {
            Thread.currentThread().interrupt();
        }
        keyRotationManager.close();
    }

    @Override
    public String toString() {
        return String.format("KeyRotationScheduler[interval=%s, running=%s]",
                "60m", !scheduler.isShutdown());
    }

    // --- Internal ---

    private void checkRotation() {
        KeyRotationManager.KeyGeneration current = keyRotationManager.getCurrentKey();
        if (current != null) {
            Duration age = Duration.between(current.getCreatedAt(), Instant.now());
            log.debug("Current key age: {} (generation {})", age, current.getGeneration());
        }
    }
}
