package com.p2p.network.tcp;

import com.p2p.core.config.AppConfig;
import com.p2p.network.protocol.*;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.net.*;
import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicBoolean;

/**
 * TCP server that accepts incoming peer connections.
 *
 * <p>Binds on all non-loopback IPv4 interfaces for multi-homing support.
 * Each accepted connection is handled on a virtual thread via
 * {@link Executors#newVirtualThreadPerTaskExecutor()}.
 *
 * <p>Connections are dispatched to a {@link ConnectionHandler} callback
 * which receives the socket and paired {@link MessageReader}/{@link MessageWriter}.
 */
public class TcpServer {

    private static final Logger log = LoggerFactory.getLogger(TcpServer.class);

    private final int port;
    private final ConnectionHandler handler;
    private final AtomicBoolean running = new AtomicBoolean(false);
    private final List<ServerSocket> serverSockets = new ArrayList<>();

    private ExecutorService acceptExecutor;

    /**
     * Callback interface for handling new TCP connections.
     */
    @FunctionalInterface
    public interface ConnectionHandler {
        /**
         * Handles a newly accepted connection.
         *
         * @param socket the connected socket
         * @param reader the message reader for this connection
         * @param writer the message writer for this connection
         * @throws IOException if an I/O error occurs during handling
         */
        void handleConnection(Socket socket, MessageReader reader, MessageWriter writer) throws IOException;
    }

    /**
     * Creates a TCP server that listens on the given port.
     *
     * @param port    the TCP port to listen on
     * @param handler the callback for incoming connections
     */
    public TcpServer(int port, ConnectionHandler handler) {
        this.port = port;
        this.handler = Objects.requireNonNull(handler, "handler must not be null");
    }

    /**
     * Creates a TCP server configured from {@link AppConfig}.
     *
     * @param config  the application configuration
     * @param handler the callback for incoming connections
     */
    public TcpServer(AppConfig config, ConnectionHandler handler) {
        this(config.getTcpPort(), handler);
    }

    // --- Lifecycle ---

    /**
     * Starts the TCP server, binding on all available non-loopback IPv4 interfaces.
     *
     * @throws IOException if binding fails on all interfaces
     */
    public void start() throws IOException {
        if (running.getAndSet(true)) {
            log.warn("TCP server already running");
            return;
        }

        acceptExecutor = Executors.newVirtualThreadPerTaskExecutor();

        List<InetAddress> addresses = new ArrayList<>();
        try {
            var netIfs = NetworkInterface.networkInterfaces();
            if (netIfs != null) {
                netIfs.forEach(ni -> {
                    try {
                        if (!ni.isLoopback() && ni.isUp()) {
                            ni.inetAddresses().forEach(addr -> {
                                if (addr instanceof Inet4Address) {
                                    addresses.add(addr);
                                }
                            });
                        }
                    } catch (SocketException e) {
                        log.debug("Error checking interface {}: {}", ni.getDisplayName(), e.getMessage());
                    }
                });
            }
        } catch (Exception e) {
            log.debug("Could not enumerate interfaces, binding to wildcard: {}", e.getMessage());
        }
        if (addresses.isEmpty()) {
            addresses.add(new InetSocketAddress(port).getAddress());
        }

        for (InetAddress addr : addresses) {
            try {
                ServerSocket ss = new ServerSocket();
                ss.setReuseAddress(true);
                ss.bind(new InetSocketAddress(addr, port));
                serverSockets.add(ss);
                Thread.ofVirtual().name("tcp-accept-" + addr.getHostAddress())
                        .start(() -> acceptLoop(ss));
                log.info("TCP server listening on {}:{}", addr.getHostAddress(), port);
            } catch (IOException e) {
                log.warn("Failed to bind on {}: {} — skipping", addr.getHostAddress(), e.getMessage());
            }
        }

        if (serverSockets.isEmpty()) {
            running.set(false);
            throw new IOException("Failed to bind TCP server on any interface");
        }
    }

    /**
     * Stops the TCP server, closing all server sockets and shutting down the
     * accept executor.
     */
    public void stop() {
        if (!running.getAndSet(false)) return;

        for (ServerSocket ss : serverSockets) {
            try {
                if (ss != null && !ss.isClosed()) ss.close();
            } catch (IOException e) {
                log.debug("Error closing server socket: {}", e.getMessage());
            }
        }
        serverSockets.clear();

        if (acceptExecutor != null) {
            acceptExecutor.shutdownNow();
        }

        log.info("TCP server stopped");
    }

    // --- State ---

    /**
     * Returns whether the server is currently running.
     *
     * @return true if the server is running
     */
    public boolean isRunning() {
        return running.get();
    }

    /**
     * Returns the port the server is listening on.
     *
     * @return the local port of the first bound server socket, or the configured port
     */
    public int getPort() {
        return !serverSockets.isEmpty() ? serverSockets.get(0).getLocalPort() : port;
    }

    // --- Internal ---

    /**
     * Accept loop for a single server socket. Runs until the server is stopped
     * or the socket is closed.
     */
    private void acceptLoop(ServerSocket ss) {
        while (running.get() && !ss.isClosed()) {
            try {
                Socket clientSocket = ss.accept();
                clientSocket.setTcpNoDelay(true);
                clientSocket.setKeepAlive(true);
                clientSocket.setSoTimeout(30000);

                log.info("Accepted connection from {}", clientSocket.getRemoteSocketAddress());

                acceptExecutor.execute(() -> handleClient(clientSocket));

            } catch (IOException e) {
                if (running.get()) {
                    log.debug("Error accepting connection: {}", e.getMessage());
                }
            }
        }
    }

    /**
     * Wraps a socket in reader/writer and dispatches to the connection handler.
     */
    private void handleClient(Socket socket) {
        try (socket) {
            MessageReader reader = new MessageReader(socket);
            MessageWriter writer = new MessageWriter(socket);
            handler.handleConnection(socket, reader, writer);
        } catch (IOException e) {
            log.debug("Connection handler error: {}", e.getMessage());
        }
    }
}
