package com.p2p.relay;

import com.sun.net.httpserver.HttpServer;
import com.sun.net.httpserver.HttpExchange;

import java.io.*;
import java.net.InetSocketAddress;
import java.nio.charset.StandardCharsets;
import java.time.Instant;
import java.util.*;
import java.util.concurrent.*;
import java.util.stream.Collectors;

/**
 * Lightweight HTTP registry server for global peer discovery.
 * Peers register their public address and discover others.
 * Runnable standalone or embedded.
 */
public final class RegistryServer implements AutoCloseable {

    private static final long PEER_EXPIRY_SECONDS = 90;

    private final HttpServer server;
    private final ConcurrentMap<String, PeerRecord> peers = new ConcurrentHashMap<>();
    private final ScheduledExecutorService cleanup = Executors.newSingleThreadScheduledExecutor();

    public record PeerRecord(
        String peerId, String displayName, int tcpPort,
        String publicIp, int publicPort, String os, String version,
        long lastHeartbeat
    ) {
        public boolean isExpired() {
            return Instant.now().toEpochMilli() - lastHeartbeat > PEER_EXPIRY_SECONDS * 1000;
        }
    }

    public RegistryServer(int port) throws IOException {
        this.server = HttpServer.create(new InetSocketAddress(port), 0);
        setupRoutes();
        cleanup.scheduleAtFixedRate(this::expirePeers, 30, 30, TimeUnit.SECONDS);
    }

    public void start() {
        server.start();
        System.out.println("Registry server listening on port " + server.getAddress().getPort());
    }

    @Override
    public void close() {
        server.stop(0);
        cleanup.shutdownNow();
    }

    public int getPort() { return server.getAddress().getPort(); }
    public List<PeerRecord> getPeers() { return List.copyOf(peers.values()); }

    // --- Routes ---

    private void setupRoutes() {
        server.createContext("/api/register", this::handleRegister);
        server.createContext("/api/peers", this::handlePeers);
        server.createContext("/api/heartbeat", this::handleHeartbeat);
        server.setExecutor(Executors.newVirtualThreadPerTaskExecutor());
    }

    private void handleRegister(HttpExchange ex) throws IOException {
        if (!"POST".equalsIgnoreCase(ex.getRequestMethod())) {
            send(ex, 405, "Method not allowed");
            return;
        }
        try {
            String body = readBody(ex);
            var peerId = extractJson(body, "peerId");
            if (peerId.isEmpty()) { send(ex, 400, "Missing peerId"); return; }

            var record = new PeerRecord(
                peerId,
                extractJson(body, "displayName"),
                Integer.parseInt(extractJson(body, "tcpPort")),
                extractJson(body, "publicIp"),
                Integer.parseInt(extractJson(body, "publicPort")),
                extractJson(body, "os"),
                extractJson(body, "version"),
                Instant.now().toEpochMilli()
            );
            peers.put(peerId, record);
            send(ex, 200, "{\"status\":\"registered\"}");
        } catch (Exception e) {
            send(ex, 400, "{\"error\":\"" + e.getMessage() + "\"}");
        }
    }

    private void handlePeers(HttpExchange ex) throws IOException {
        String method = ex.getRequestMethod().toUpperCase();
        String path = ex.getRequestURI().getPath();

        if ("GET".equals(method)) {
            String peerId = path.replace("/api/peers/", "").replace("/api/peers", "");
            if (!peerId.isEmpty()) {
                var p = peers.get(peerId);
                if (p == null) { send(ex, 404, "{\"error\":\"not found\"}"); return; }
                send(ex, 200, toJson(p));
            } else {
                expirePeers();
                var list = peers.values().stream()
                        .filter(p -> !p.isExpired())
                        .map(this::toJson)
                        .collect(Collectors.joining(",", "[", "]"));
                send(ex, 200, list);
            }
        } else if ("DELETE".equals(method)) {
            String peerId = path.replace("/api/register/", "").replace("/api/peers/", "");
            peers.remove(peerId);
            send(ex, 200, "{\"status\":\"removed\"}");
        } else {
            send(ex, 405, "Method not allowed");
        }
    }

    private void handleHeartbeat(HttpExchange ex) throws IOException {
        if (!"POST".equalsIgnoreCase(ex.getRequestMethod())) {
            send(ex, 405, "Method not allowed");
            return;
        }
        try {
            String body = readBody(ex);
            String peerId = extractJson(body, "peerId");
            var existing = peers.get(peerId);
            if (existing == null) { send(ex, 404, "{\"error\":\"not found\"}"); return; }
            peers.put(peerId, new PeerRecord(
                existing.peerId(), existing.displayName(), existing.tcpPort(),
                existing.publicIp(), existing.publicPort(), existing.os(),
                existing.version(), Instant.now().toEpochMilli()
            ));
            send(ex, 200, "{\"status\":\"ok\"}");
        } catch (Exception e) {
            send(ex, 400, "{\"error\":\"" + e.getMessage() + "\"}");
        }
    }

    private void expirePeers() {
        peers.values().removeIf(PeerRecord::isExpired);
    }

    // --- Helpers ---

    private String toJson(PeerRecord p) {
        return String.format(
            "{\"peerId\":\"%s\",\"displayName\":\"%s\",\"tcpPort\":%d," +
            "\"publicIp\":\"%s\",\"publicPort\":%d,\"os\":\"%s\",\"version\":\"%s\"}",
            p.peerId(), p.displayName(), p.tcpPort(),
            p.publicIp(), p.publicPort(), p.os(), p.version()
        );
    }

    private static String readBody(HttpExchange ex) throws IOException {
        try (var is = ex.getRequestBody()) {
            return new String(is.readAllBytes(), StandardCharsets.UTF_8);
        }
    }

    private static void send(HttpExchange ex, int code, String body) throws IOException {
        byte[] bytes = body.getBytes(StandardCharsets.UTF_8);
        ex.getResponseHeaders().add("Content-Type", "application/json");
        ex.getResponseHeaders().add("Access-Control-Allow-Origin", "*");
        ex.getResponseHeaders().add("Access-Control-Allow-Methods", "GET, POST, DELETE, OPTIONS");
        ex.getResponseHeaders().add("Access-Control-Allow-Headers", "Content-Type");
        if ("OPTIONS".equalsIgnoreCase(ex.getRequestMethod())) {
            ex.sendResponseHeaders(204, -1);
            return;
        }
        ex.sendResponseHeaders(code, bytes.length);
        try (var os = ex.getResponseBody()) {
            os.write(bytes);
        }
    }

    private static String extractJson(String json, String key) {
        int idx = json.indexOf("\"" + key + "\"");
        if (idx < 0) return "";
        int colon = json.indexOf(':', idx + key.length() + 2);
        if (colon < 0) return "";
        int start = colon + 1;
        while (start < json.length() && Character.isWhitespace(json.charAt(start))) start++;
        if (start >= json.length()) return "";
        if (json.charAt(start) == '"') {
            start++;
            int end = start;
            while (end < json.length() && json.charAt(end) != '"') end++;
            return json.substring(start, end);
        }
        int end = start;
        while (end < json.length() && json.charAt(end) != ',' && json.charAt(end) != '}' && json.charAt(end) != ']') end++;
        return json.substring(start, end).trim();
    }

    // --- Entry point ---

    public static void main(String[] args) throws Exception {
        int port = args.length > 0 ? Integer.parseInt(args[0]) : 8080;
        var server = new RegistryServer(port);
        server.start();
        Runtime.getRuntime().addShutdownHook(new Thread(() -> {
            server.close();
            System.out.println("Registry server stopped");
        }));
    }
}

