package com.p2p.observability.metrics;

import com.sun.net.httpserver.HttpServer;
import io.micrometer.prometheusmetrics.PrometheusConfig;
import io.micrometer.prometheusmetrics.PrometheusMeterRegistry;
import io.micrometer.core.instrument.*;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.io.OutputStream;
import java.net.InetSocketAddress;
import java.util.Objects;
import java.util.concurrent.Executors;

/**
 * Prometheus-compatible metrics exporter backed by Micrometer.
 * Exposes metrics at the {@code /metrics} HTTP endpoint in Prometheus text format.
 */
public final class MetricsExporter implements AutoCloseable {

    // --- Constants ---

    private static final Logger log = LoggerFactory.getLogger(MetricsExporter.class);

    // --- Fields ---

    private final PrometheusMeterRegistry registry;
    private final HttpServer server;
    private volatile boolean running;

    // --- Constructor ---

    /**
     * Creates but does not start the metrics exporter.
     *
     * @param port the HTTP port to bind the metrics endpoint to
     * @throws IOException if the HTTP server cannot be created
     */
    public MetricsExporter(int port) throws IOException {
        this.registry = new PrometheusMeterRegistry(PrometheusConfig.DEFAULT);
        this.server = HttpServer.create(new InetSocketAddress(port), 0);
        server.setExecutor(Executors.newVirtualThreadPerTaskExecutor());
        server.createContext("/metrics", exchange -> {
            String response = registry.scrape();
            exchange.getResponseHeaders().set("Content-Type", "text/plain; version=0.0.4");
            byte[] responseBytes = response.getBytes();
            exchange.sendResponseHeaders(200, responseBytes.length);
            try (OutputStream os = exchange.getResponseBody()) {
                os.write(responseBytes);
            }
        });
    }

    // --- Public API ---

    /**
     * Starts the HTTP metrics server.
     */
    public void start() {
        server.start();
        running = true;
        log.info("Metrics exporter started on port {}", server.getAddress().toString());
    }

    /**
     * Stops the HTTP metrics server with a grace period of 5 seconds.
     */
    public void stop() {
        if (running) {
            server.stop(5);
            running = false;
            log.info("Metrics exporter stopped");
        }
    }

    /**
     * Returns the underlying Micrometer registry for direct meter access.
     *
     * @return the PrometheusMeterRegistry instance
     */
    public PrometheusMeterRegistry getRegistry() {
        return registry;
    }

    /**
     * Creates and registers a {@link Counter} with the given name and description.
     *
     * @param name        the metric name (must not be null or empty)
     * @param description a human-readable description (must not be null)
     * @return the registered Counter
     */
    public Counter counter(String name, String description) {
        Objects.requireNonNull(name, "name must not be null");
        Objects.requireNonNull(description, "description must not be null");
        return Counter.builder(name).description(description).register(registry);
    }

    /**
     * Creates and registers a {@link Gauge} backed by the given supplier.
     *
     * @param name        the metric name (must not be null or empty)
     * @param description a human-readable description (must not be null)
     * @param supplier    a supplier of the gauge's current value (must not be null)
     * @return the registered Gauge
     */
    public Gauge gauge(String name, String description, NumberSupplier supplier) {
        Objects.requireNonNull(name, "name must not be null");
        Objects.requireNonNull(description, "description must not be null");
        Objects.requireNonNull(supplier, "supplier must not be null");
        return Gauge.builder(name, supplier).description(description).register(registry);
    }

    /**
     * Creates and registers a {@link Timer} with the given name and description.
     *
     * @param name        the metric name (must not be null or empty)
     * @param description a human-readable description (must not be null)
     * @return the registered Timer
     */
    public Timer timer(String name, String description) {
        Objects.requireNonNull(name, "name must not be null");
        Objects.requireNonNull(description, "description must not be null");
        return Timer.builder(name).description(description).register(registry);
    }

    /**
     * Creates and registers a {@link DistributionSummary} with the given name and description.
     *
     * @param name        the metric name (must not be null or empty)
     * @param description a human-readable description (must not be null)
     * @return the registered DistributionSummary
     */
    public DistributionSummary summary(String name, String description) {
        Objects.requireNonNull(name, "name must not be null");
        Objects.requireNonNull(description, "description must not be null");
        return DistributionSummary.builder(name).description(description).register(registry);
    }

    /**
     * Records a completed transfer as a counter increment and timer sample.
     *
     * @param fileSizeBytes the size of the transferred file in bytes (must be non-negative)
     * @param durationMs    the transfer duration in milliseconds (must be non-negative)
     */
    public void recordTransfer(long fileSizeBytes, long durationMs) {
        if (fileSizeBytes < 0) {
            throw new IllegalArgumentException("fileSizeBytes must be non-negative, got: " + fileSizeBytes);
        }
        if (durationMs < 0) {
            throw new IllegalArgumentException("durationMs must be non-negative, got: " + durationMs);
        }
        counter("p2p.transfer.bytes", "Total bytes transferred").increment(fileSizeBytes);
        counter("p2p.transfer.count", "Total transfer count").increment();
        timer("p2p.transfer.duration", "Transfer duration").record(java.time.Duration.ofMillis(durationMs));
    }

    /**
     * Records the current number of connected peers.
     *
     * @param count the peer count (must be non-negative)
     */
    public void recordConnectedPeers(int count) {
        gauge("p2p.peers.connected", "Currently connected peers", () -> count);
    }

    @Override
    public void close() {
        stop();
        registry.close();
    }

    // --- Functional interfaces ---

    /**
     * Supplier of {@link Number} values, primarily for gauge callbacks.
     */
    @FunctionalInterface
    public interface NumberSupplier extends java.util.function.Supplier<Number> {}
}
