package com.p2p.relay;

import com.p2p.core.model.PeerId;
import com.p2p.core.model.PeerInfo;
import com.p2p.core.model.PeerStatus;
import com.p2p.core.util.PlatformUtils;

import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.net.http.HttpRequest.BodyPublishers;
import java.net.http.HttpResponse.BodyHandlers;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.time.Instant;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;

/**
 * HTTP client for the global P2P registry server.
 * Peers register their public address so others can find them across the internet.
 */
public final class RegistryClient {

    private static final Duration TIMEOUT = Duration.ofSeconds(5);
    private static final long HEARTBEAT_SECONDS = 30;

    private final HttpClient http;
    private final String registryUrl;
    private volatile boolean registered;

    public RegistryClient() {
        this("https://registry.p2ptransfer.io");
    }

    public RegistryClient(String registryUrl) {
        this.http = HttpClient.newBuilder()
                .connectTimeout(TIMEOUT)
                .build();
        this.registryUrl = registryUrl.replaceAll("/+$", "");
    }

    public boolean register(PeerId peerId, String displayName, int tcpPort,
                            String publicIp, int publicPort) {
        try {
            String json = String.format(
                "{\"peerId\":\"%s\",\"displayName\":\"%s\",\"tcpPort\":%d," +
                "\"publicIp\":\"%s\",\"publicPort\":%d,\"os\":\"%s\",\"version\":\"%s\"}",
                peerId.toHex(), escape(displayName), tcpPort,
                publicIp != null ? publicIp : "", publicPort,
                escape(PlatformUtils.getPlatformString()), "1.0.0"
            );
            var req = HttpRequest.newBuilder()
                    .uri(URI.create(registryUrl + "/api/register"))
                    .header("Content-Type", "application/json")
                    .POST(BodyPublishers.ofString(json, StandardCharsets.UTF_8))
                    .timeout(TIMEOUT)
                    .build();
            var resp = http.send(req, BodyHandlers.ofString());
            registered = resp.statusCode() == 200 || resp.statusCode() == 201;
            return registered;
        } catch (Exception e) {
            return false;
        }
    }

    public void unregister(String peerId) {
        try {
            var req = HttpRequest.newBuilder()
                    .uri(URI.create(registryUrl + "/api/register/" + peerId))
                    .DELETE()
                    .timeout(TIMEOUT)
                    .build();
            http.send(req, BodyHandlers.discarding());
        } catch (Exception ignored) {}
        registered = false;
    }

    public List<PeerInfo> listPeers() {
        try {
            var req = HttpRequest.newBuilder()
                    .uri(URI.create(registryUrl + "/api/peers"))
                    .GET()
                    .timeout(TIMEOUT)
                    .build();
            var resp = http.send(req, BodyHandlers.ofString());
            if (resp.statusCode() != 200) return List.of();
            return parsePeerList(resp.body());
        } catch (Exception e) {
            return List.of();
        }
    }

    public Optional<PeerInfo> findPeer(String peerId) {
        try {
            var req = HttpRequest.newBuilder()
                    .uri(URI.create(registryUrl + "/api/peers/" + peerId))
                    .GET()
                    .timeout(TIMEOUT)
                    .build();
            var resp = http.send(req, BodyHandlers.ofString());
            if (resp.statusCode() != 200) return Optional.empty();
            return Optional.ofNullable(parsePeer(resp.body()));
        } catch (Exception e) {
            return Optional.empty();
        }
    }

    public boolean heartbeat(PeerId peerId) {
        try {
            var json = "{\"peerId\":\"" + peerId.toHex() + "\"}";
            var req = HttpRequest.newBuilder()
                    .uri(URI.create(registryUrl + "/api/heartbeat"))
                    .header("Content-Type", "application/json")
                    .POST(BodyPublishers.ofString(json, StandardCharsets.UTF_8))
                    .timeout(TIMEOUT)
                    .build();
            return http.send(req, BodyHandlers.discarding()).statusCode() == 200;
        } catch (Exception e) {
            return false;
        }
    }

    public boolean isRegistered() { return registered; }
    public long getHeartbeatInterval() { return HEARTBEAT_SECONDS; }

    private List<PeerInfo> parsePeerList(String json) {
        var list = new ArrayList<PeerInfo>();
        try {
            var parser = new com.p2p.core.util.JsonArrayParser(json);
            while (parser.hasNext()) {
                var obj = parser.nextObject();
                var p = parsePeerObject(obj);
                if (p != null) list.add(p);
            }
        } catch (Exception ignored) {}
        return list;
    }

    private PeerInfo parsePeer(String json) {
        return parsePeerObject(json);
    }

    private PeerInfo parsePeerObject(String obj) {
        try {
            var peerId = PeerId.fromHex(extractJson(obj, "peerId"));
            var displayName = extractJson(obj, "displayName");
            var ip = extractJson(obj, "publicIp");
            int port = Integer.parseInt(extractJson(obj, "publicPort"));
            var os = extractJson(obj, "os");
            if (ip.isEmpty() || port == 0) return null;
            return PeerInfo.builder()
                    .peerId(peerId)
                    .displayName(displayName)
                    .address(java.net.InetAddress.getByName(ip))
                    .port(port)
                    .operatingSystem(os)
                    .appVersion("1.0.0")
                    .lastSeen(Instant.now())
                    .status(PeerStatus.ONLINE)
                    .build();
        } catch (Exception e) {
            return null;
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

    private static String escape(String s) {
        return s.replace("\\", "\\\\").replace("\"", "\\\"");
    }
}

