package com.p2p.relay;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.*;
import java.net.*;
import java.util.concurrent.*;

/**
 * Simple TCP relay server for fallback when direct P2P connections fail.
 * Both peers connect to this server, which bridges their streams.
 */
public final class RelayServer implements AutoCloseable {

    private static final Logger log = LoggerFactory.getLogger(RelayServer.class);
    private static final int BUFFER_SIZE = 65536;
    private static final long RELAY_TIMEOUT_MS = 300_000;

    private final ServerSocket serverSocket;
    private final ExecutorService executor;
    private final ConcurrentMap<String, RelaySession> sessions = new ConcurrentHashMap<>();
    private volatile boolean running;

    public RelayServer(int port) throws IOException {
        this.serverSocket = new ServerSocket(port);
        this.executor = Executors.newVirtualThreadPerTaskExecutor();
    }

    public void start() {
        running = true;
        log.info("Relay server listening on port {}", serverSocket.getLocalPort());
        executor.submit(this::acceptLoop);
    }

    @Override
    public void close() {
        running = false;
        try { serverSocket.close(); } catch (IOException ignored) {}
        executor.shutdownNow();
    }

    public int getPort() { return serverSocket.getLocalPort(); }

    public record RelaySession(String sessionId, Socket peerA, Socket peerB) {}

    // --- Accept loop ---

    private void acceptLoop() {
        while (running) {
            try {
                var sock = serverSocket.accept();
                executor.submit(() -> handlePeer(sock));
            } catch (IOException e) {
                if (running) log.error("Accept error: {}", e.getMessage());
            }
        }
    }

    /**
     * First peer sends RELAY_REQUEST with a sessionId.
     * Second peer sends RELAY_ACCEPT with same sessionId → bridge formed.
     */
    private void handlePeer(Socket sock) {
        try {
            var dis = new DataInputStream(sock.getInputStream());
            var dos = new DataOutputStream(sock.getOutputStream());
            String cmd = dis.readUTF();
            String sessionId = dis.readUTF();

            if ("RELAY_REQUEST".equals(cmd)) {
                sessions.put(sessionId, new RelaySession(sessionId, sock, null));
                log.info("Relay session {} waiting for second peer", sessionId);
                dos.writeUTF("WAITING");
                long deadline = System.currentTimeMillis() + 30_000;
                while (System.currentTimeMillis() < deadline) {
                    var s = sessions.get(sessionId);
                    if (s != null && s.peerB() != null) {
                        bridge(s.peerA(), s.peerB());
                        return;
                    }
                    Thread.sleep(100);
                }
                sessions.remove(sessionId);
                sock.close();
                log.info("Relay session {} timed out", sessionId);
            } else if ("RELAY_ACCEPT".equals(cmd)) {
                var s = sessions.get(sessionId);
                if (s != null && s.peerB() == null) {
                    sessions.put(sessionId, new RelaySession(sessionId, s.peerA(), sock));
                    dos.writeUTF("CONNECTED");
                    bridge(s.peerA(), sock);
                } else {
                    dos.writeUTF("SESSION_NOT_FOUND");
                    sock.close();
                }
            } else {
                dos.writeUTF("UNKNOWN_COMMAND");
                sock.close();
            }
        } catch (Exception e) {
            try { sock.close(); } catch (IOException ignored) {}
        }
    }

    private void bridge(Socket a, Socket b) {
        log.info("Bridging {} ↔ {}", a.getRemoteSocketAddress(), b.getRemoteSocketAddress());
        var futureA = executor.submit(() -> {
            relay(a.getInputStream(), b.getOutputStream());
            return null;
        });
        var futureB = executor.submit(() -> {
            relay(b.getInputStream(), a.getOutputStream());
            return null;
        });
        try {
            futureA.get(RELAY_TIMEOUT_MS, TimeUnit.MILLISECONDS);
        } catch (Exception ignored) {}
        try {
            futureB.get(2, TimeUnit.SECONDS);
        } catch (Exception ignored) {}
        try { a.close(); } catch (IOException ignored) {}
        try { b.close(); } catch (IOException ignored) {}
    }

    private void relay(InputStream in, OutputStream out) {
        try {
            byte[] buf = new byte[BUFFER_SIZE];
            int read;
            while ((read = in.read(buf)) != -1) {
                out.write(buf, 0, read);
                out.flush();
            }
        } catch (IOException ignored) {}
    }

    // --- Entry point ---

    public static void main(String[] args) throws Exception {
        int port = args.length > 0 ? Integer.parseInt(args[0]) : 9878;
        var server = new RelayServer(port);
        server.start();
        System.out.println("Relay server running on port " + port + " (Ctrl+C to stop)");
        Thread.sleep(Long.MAX_VALUE);
    }
}

