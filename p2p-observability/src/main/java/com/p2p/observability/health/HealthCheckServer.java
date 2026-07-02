package com.p2p.observability.health;

import com.p2p.core.model.PeerId;
import com.p2p.core.util.NetworkUtils;
import com.p2p.core.util.PlatformUtils;
import com.sun.net.httpserver.HttpServer;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.io.OutputStream;
import java.lang.management.ManagementFactory;
import java.lang.management.OperatingSystemMXBean;
import java.net.InetSocketAddress;
import java.nio.file.FileStore;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Objects;
import java.util.concurrent.Executors;
import java.util.concurrent.atomic.AtomicReference;
import java.util.stream.Collectors;

/**
 * Lightweight HTTP health check server that exposes JSON status at {@code /health}.
 * Reports peer identity, uptime, network interfaces, disk space, system load,
 * active sessions, and connected peer count.
 */
public final class HealthCheckServer implements AutoCloseable {

    // --- Constants ---

    private static final Logger log = LoggerFactory.getLogger(HealthCheckServer.class);

    // --- Fields ---

    private final HttpServer server;
    private final PeerId localPeerId;
    private final long startTime;
    private final Path dataDir;
    private final AtomicReference<Integer> activeSessions = new AtomicReference<>(0);
    private final AtomicReference<Integer> connectedPeers = new AtomicReference<>(0);

    // --- Constructor ---

    /**
     * Creates but does not start the health check server.
     *
     * @param port        the HTTP port to bind to
     * @param localPeerId this peer's identity (must not be null)
     * @param dataDir     the data directory for disk space reporting (must not be null)
     * @throws IOException if the server cannot be created
     */
    public HealthCheckServer(int port, PeerId localPeerId, Path dataDir) throws IOException {
        Objects.requireNonNull(localPeerId, "localPeerId must not be null");
        Objects.requireNonNull(dataDir, "dataDir must not be null");
        this.server = HttpServer.create(new InetSocketAddress(port), 0);
        this.server.setExecutor(Executors.newVirtualThreadPerTaskExecutor());
        this.localPeerId = localPeerId;
        this.startTime = System.currentTimeMillis();
        this.dataDir = dataDir;
        setupContext();
    }

    // --- Public API ---

    /**
     * Starts the HTTP server on the configured port.
     */
    public void start() {
        server.start();
        log.info("Health check server started on port {}", server.getAddress().getPort());
    }

    /**
     * Stops the HTTP server with a grace period of 5 seconds.
     */
    public void stop() {
        server.stop(5);
        log.info("Health check server stopped");
    }

    /**
     * Updates the active transfer session count reported in health JSON.
     *
     * @param count the number of active sessions
     */
    public void setActiveSessions(int count) {
        activeSessions.set(count);
    }

    /**
     * Updates the connected peer count reported in health JSON.
     *
     * @param count the number of connected peers
     */
    public void setConnectedPeers(int count) {
        connectedPeers.set(count);
    }

    /**
     * Returns whether the server is currently bound and accepting requests.
     *
     * @return true if the server is running
     */
    public boolean isHealthy() {
        return server.getAddress() != null;
    }

    @Override
    public void close() {
        stop();
    }

    // --- Private helpers ---

    private void setupContext() {
        server.createContext("/health", exchange -> {
            String json = buildHealthJson();
            byte[] response = json.getBytes();
            exchange.getResponseHeaders().set("Content-Type", "application/json");
            exchange.sendResponseHeaders(200, response.length);
            try (OutputStream os = exchange.getResponseBody()) {
                os.write(response);
            }
        });
    }

    private String buildHealthJson() {
        long uptime = (System.currentTimeMillis() - startTime) / 1000;
        String diskSpace = getDiskSpace();
        String load = getSystemLoad();
        List<String> interfaces = NetworkUtils.getActiveInterfaces().stream()
                .map(ni -> ni.getDisplayName() + " (" + ni.getName() + ")")
                .collect(Collectors.toList());

        return String.format("""
                {
                  "status": "UP",
                  "peerId": "%s",
                  "uptime": "%ds",
                  "networkInterfaces": %s,
                  "diskSpaceAvailable": "%s",
                  "systemLoad": "%s",
                  "activeSessions": %d,
                  "connectedPeers": %d
                }""",
                localPeerId.toShortString(), uptime,
                toJsonArray(interfaces), diskSpace, load,
                activeSessions.get(), connectedPeers.get());
    }

    private String getDiskSpace() {
        try {
            if (dataDir != null && Files.exists(dataDir)) {
                FileStore store = Files.getFileStore(dataDir);
                long usable = store.getUsableSpace();
                return formatBytes(usable);
            }
        } catch (IOException e) {
            return "unknown";
        }
        return "unknown";
    }

    private String getSystemLoad() {
        OperatingSystemMXBean os = ManagementFactory.getOperatingSystemMXBean();
        return String.format("%.2f%%", os.getSystemLoadAverage() * 100);
    }

    private String toJsonArray(List<String> items) {
        return items.stream().map(s -> "\"" + s.replace("\"", "\\\"") + "\"")
                .collect(Collectors.joining(", ", "[", "]"));
    }

    private String formatBytes(long bytes) {
        if (bytes < 1024) return bytes + " B";
        if (bytes < 1024 * 1024) return String.format("%.1f KB", bytes / 1024.0);
        if (bytes < 1024 * 1024 * 1024) return String.format("%.1f MB", bytes / (1024.0 * 1024));
        return String.format("%.2f GB", bytes / (1024.0 * 1024 * 1024));
    }
}
