package com.p2p.relay;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.*;
import java.net.Socket;
import java.util.UUID;

/**
 * Client for connecting through the RelayServer fallback.
 * Both peers connect to the same relay session — one as REQUEST, one as ACCEPT.
 */
public final class RelayClient {

    private static final Logger log = LoggerFactory.getLogger(RelayClient.class);
    private static final int CONNECT_TIMEOUT_MS = 10000;

    private final String relayHost;
    private final int relayPort;

    public RelayClient(String relayHost, int relayPort) {
        this.relayHost = relayHost;
        this.relayPort = relayPort;
    }

    /**
     * Connect as the initiating peer (creates a new relay session).
     */
    public Socket connectAsInitiator() throws IOException {
        String sessionId = UUID.randomUUID().toString().replace("-", "");
        return connectToSession(sessionId, "RELAY_REQUEST");
    }

    /**
     * Connect as the accepting peer (joins an existing relay session).
     */
    public Socket connectAsAcceptor(String sessionId) throws IOException {
        return connectToSession(sessionId, "RELAY_ACCEPT");
    }

    private Socket connectToSession(String sessionId, String cmd) throws IOException {
        var sock = new Socket();
        sock.setTcpNoDelay(true);
        sock.setKeepAlive(true);
        sock.connect(new java.net.InetSocketAddress(relayHost, relayPort), CONNECT_TIMEOUT_MS);
        sock.setSoTimeout(30000);

        var dos = new DataOutputStream(sock.getOutputStream());
        var dis = new DataInputStream(sock.getInputStream());

        dos.writeUTF(cmd);
        dos.writeUTF(sessionId);
        dos.flush();

        String response = dis.readUTF();
        if ("CONNECTED".equals(response) || "WAITING".equals(response)) {
            log.debug("Relay session {}: {}", sessionId, response);
            return sock;
        }
        sock.close();
        throw new IOException("Relay rejected: " + response);
    }

    public String generateSessionId() {
        return UUID.randomUUID().toString().replace("-", "");
    }
}

