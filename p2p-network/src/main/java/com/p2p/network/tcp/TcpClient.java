package com.p2p.network.tcp;

import com.p2p.network.protocol.*;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.net.*;
import java.util.Objects;

/**
 * TCP client for establishing outgoing connections to remote peers.
 *
 * <p>Provides both a raw socket connect and a higher-level
 * {@link #connectWithProtocol(InetAddress, int)} that returns a
 * {@link Connection} record with paired {@link MessageReader}/{@link MessageWriter}.
 *
 * <p>Sockets are configured with TCP_NODELAY, keepalive, and a configurable
 * SO_TIMEOUT for read operations.
 */
public class TcpClient {

    private static final Logger log = LoggerFactory.getLogger(TcpClient.class);

    // --- Constants ---

    private static final int DEFAULT_CONNECT_TIMEOUT_MS = 10000;
    private static final int DEFAULT_SO_TIMEOUT_MS = 30000;

    private final int connectTimeoutMs;
    private final int soTimeoutMs;

    /**
     * Creates a client with default timeouts (10s connect, 30s socket).
     */
    public TcpClient() {
        this(DEFAULT_CONNECT_TIMEOUT_MS, DEFAULT_SO_TIMEOUT_MS);
    }

    /**
     * Creates a client with custom timeouts.
     *
     * @param connectTimeoutMs connection timeout in milliseconds
     * @param soTimeoutMs      socket read timeout in milliseconds
     */
    public TcpClient(int connectTimeoutMs, int soTimeoutMs) {
        this.connectTimeoutMs = connectTimeoutMs;
        this.soTimeoutMs = soTimeoutMs;
    }

    // --- Connect ---

    /**
     * Establishes a raw TCP connection to a remote peer.
     *
     * <p>The returned socket has TCP_NODELAY enabled, keepalive enabled, and
     * SO_TIMEOUT set to the configured value.
     *
     * @param address the peer's IP address (must not be null)
     * @param port    the peer's TCP port
     * @return the connected socket
     * @throws IOException if the connection fails
     */
    public Socket connect(InetAddress address, int port) throws IOException {
        Objects.requireNonNull(address, "address must not be null");

        log.info("Connecting to {}:{}", address.getHostAddress(), port);

        Socket socket = new Socket();
        socket.setTcpNoDelay(true);
        socket.setKeepAlive(true);
        socket.setSoTimeout(soTimeoutMs);

        try {
            socket.connect(new InetSocketAddress(address, port), connectTimeoutMs);
            log.info("Connected to {}:{}", address.getHostAddress(), port);
            return socket;
        } catch (IOException e) {
            socket.close();
            throw e;
        }
    }

    /**
     * Establishes a connection and returns a {@link Connection} with
     * paired {@link MessageReader} and {@link MessageWriter}.
     *
     * @param address the peer's IP address
     * @param port    the peer's TCP port
     * @return a Connection record wrapping the socket, reader, and writer
     * @throws IOException if the connection or stream setup fails
     */
    public Connection connectWithProtocol(InetAddress address, int port) throws IOException {
        Socket socket = connect(address, port);
        return new Connection(socket, new MessageReader(socket), new MessageWriter(socket));
    }

    /**
     * Represents an established protocol connection with paired reader/writer.
     *
     * @param socket the connected socket
     * @param reader the message reader for this connection
     * @param writer the message writer for this connection
     */
    public record Connection(Socket socket, MessageReader reader, MessageWriter writer) implements AutoCloseable {
        @Override
        public void close() throws IOException {
            try {
                writer.close();
            } finally {
                try {
                    reader.close();
                } finally {
                    socket.close();
                }
            }
        }
    }
}
