package com.p2p.network.tcp;

import com.p2p.core.model.PeerId;
import com.p2p.network.protocol.*;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.net.Socket;
import java.time.Instant;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicBoolean;

/**
 * Manages active TCP connections with heartbeat-based health monitoring.
 *
 * <p>Each peer connection is tracked as a {@link ManagedConnection} with
 * heartbeat send/receive tracking. The monitor periodically sends heartbeat
 * frames on each connection and removes peers whose writes fail.
 *
 * <p>Connections are not aggressively disconnected on missed heartbeats;
 * instead, the monitor relies on write failure detection. This avoids
 * race conditions with the shared reader/writer heartbeat filtering.
 */
public class ConnectionManager {

    private static final Logger log = LoggerFactory.getLogger(ConnectionManager.class);

    private final ConcurrentMap<PeerId, ManagedConnection> connections = new ConcurrentHashMap<>();
    private final long heartbeatIntervalMs;
    private final int missedHeartbeatsThreshold;
    private final ScheduledExecutorService heartbeatScheduler;
    private final AtomicBoolean running = new AtomicBoolean(false);

    /**
     * Creates a connection manager with the given heartbeat parameters.
     *
     * @param heartbeatIntervalMs      interval between heartbeat sends in milliseconds
     * @param missedHeartbeatsThreshold number of missed heartbeats before considering a peer dead
     */
    public ConnectionManager(long heartbeatIntervalMs, int missedHeartbeatsThreshold) {
        this.heartbeatIntervalMs = heartbeatIntervalMs;
        this.missedHeartbeatsThreshold = missedHeartbeatsThreshold;
        this.heartbeatScheduler = Executors.newSingleThreadScheduledExecutor(r -> {
            Thread t = new Thread(r, "heartbeat-monitor");
            t.setDaemon(true);
            return t;
        });
    }

    // --- Lifecycle ---

    /**
     * Starts the heartbeat monitoring background task.
     */
    public void start() {
        if (running.getAndSet(true)) return;
        heartbeatScheduler.scheduleAtFixedRate(this::checkHeartbeats,
                heartbeatIntervalMs, heartbeatIntervalMs, TimeUnit.MILLISECONDS);
        log.info("Connection manager started (heartbeat: {}ms, threshold: {})",
                heartbeatIntervalMs, missedHeartbeatsThreshold);
    }

    /**
     * Stops heartbeat monitoring and closes all managed connections.
     */
    public void stop() {
        if (!running.getAndSet(false)) return;
        heartbeatScheduler.shutdownNow();
        connections.values().forEach(this::closeQuietly);
        connections.clear();
        log.info("Connection manager stopped");
    }

    // --- Connection management ---

    /**
     * Registers a new connection for a peer. If the peer already has a connection,
     * the old one is closed and replaced.
     *
     * @param peerId the peer identifier (must not be null)
     * @param socket the connected socket (must not be null)
     * @param reader the message reader (must not be null)
     * @param writer the message writer (must not be null)
     */
    public void register(PeerId peerId, Socket socket, MessageReader reader, MessageWriter writer) {
        Objects.requireNonNull(peerId, "peerId must not be null");
        Objects.requireNonNull(socket, "socket must not be null");
        Objects.requireNonNull(reader, "reader must not be null");
        Objects.requireNonNull(writer, "writer must not be null");

        ManagedConnection conn = new ManagedConnection(peerId, socket, reader, writer);
        ManagedConnection old = connections.put(peerId, conn);
        if (old != null) {
            closeQuietly(old);
        }
        log.info("Registered connection for peer {}", peerId.toShortString());
    }

    /**
     * Gets the managed connection for a peer if it exists and is still open.
     * Stale (closed) connections are automatically removed.
     *
     * @param peerId the peer identifier
     * @return an Optional containing the connection, or empty if not found or closed
     */
    public Optional<ManagedConnection> getConnection(PeerId peerId) {
        ManagedConnection conn = connections.get(peerId);
        if (conn != null && conn.isClosed()) {
            connections.remove(peerId);
            return Optional.empty();
        }
        return Optional.ofNullable(conn);
    }

    /**
     * Removes and closes the connection for a peer.
     *
     * @param peerId the peer identifier
     */
    public void disconnect(PeerId peerId) {
        ManagedConnection conn = connections.remove(peerId);
        if (conn != null) {
            closeQuietly(conn);
            log.info("Disconnected peer {}", peerId.toShortString());
        }
    }

    // --- Heartbeat monitoring ---

    /**
     * Records that a heartbeat was received from the given peer, resetting
     * its missed-count to zero.
     *
     * @param peerId the peer that sent the heartbeat
     */
    public void recordHeartbeat(PeerId peerId) {
        ManagedConnection conn = connections.get(peerId);
        if (conn != null) {
            conn.recordHeartbeat();
        }
    }

    /**
     * Returns the number of currently registered connections.
     *
     * @return the active connection count
     */
    public int getActiveCount() {
        return connections.size();
    }

    /**
     * Periodic heartbeat check: sends a heartbeat frame on each connection.
     * If the write fails, the peer is considered unreachable and is removed.
     */
    private void checkHeartbeats() {
        for (var iter = connections.entrySet().iterator(); iter.hasNext();) {
            var entry = iter.next();
            ManagedConnection conn = entry.getValue();

            if (conn.isClosed()) {
                log.debug("Heartbeat: peer {} socket is closed, removing",
                        entry.getKey().toShortString());
                iter.remove();
                closeQuietly(conn);
                continue;
            }

            // Send a lightweight heartbeat frame. The reader on the other side
            // filters these transparently (MessageReader skips HEARTBEAT frames).
            try {
                conn.writer().writeFrame(MessageFrame.heartbeat());
            } catch (IOException e) {
                log.debug("Heartbeat send failed for {}: {}",
                        entry.getKey().toShortString(), e.getMessage());
                iter.remove();
                closeQuietly(conn);
            }
        }
    }

    /**
     * Closes a managed connection silently, swallowing any exceptions.
     */
    private void closeQuietly(ManagedConnection conn) {
        try {
            conn.close();
        } catch (Exception ignored) {
        }
    }

    // --- ManagedConnection ---

    /**
     * Represents a managed peer connection with heartbeat tracking state.
     */
    public static final class ManagedConnection implements AutoCloseable {
        private final PeerId peerId;
        private final Socket socket;
        private final MessageReader reader;
        private final MessageWriter writer;
        private volatile Instant lastHeartbeat;
        private volatile int missedCount;

        ManagedConnection(PeerId peerId, Socket socket, MessageReader reader, MessageWriter writer) {
            this.peerId = peerId;
            this.socket = socket;
            this.reader = reader;
            this.writer = writer;
            this.lastHeartbeat = Instant.now();
            this.missedCount = 0;
        }

        public PeerId peerId() {
            return peerId;
        }

        public Socket socket() {
            return socket;
        }

        public MessageReader reader() {
            return reader;
        }

        public MessageWriter writer() {
            return writer;
        }

        public Instant getLastHeartbeat() {
            return lastHeartbeat;
        }

        public int getMissedCount() {
            return missedCount;
        }

        /**
         * Records a received heartbeat, resetting the missed count.
         */
        public void recordHeartbeat() {
            this.lastHeartbeat = Instant.now();
            this.missedCount = 0;
        }

        /**
         * Increments the missed heartbeat counter.
         */
        public void incrementMissed() {
            this.missedCount++;
        }

        /**
         * Returns true if the underlying socket is closed or disconnected.
         *
         * @return true if the connection is closed
         */
        public boolean isClosed() {
            return socket.isClosed() || !socket.isConnected();
        }

        @Override
        public void close() throws IOException {
            try {
                reader.close();
            } catch (IOException ignored) {
            }
            try {
                writer.close();
            } catch (IOException ignored) {
            }
            socket.close();
        }
    }
}
