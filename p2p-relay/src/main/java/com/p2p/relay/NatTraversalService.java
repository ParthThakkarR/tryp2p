package com.p2p.relay;

import com.p2p.core.model.PeerInfo;
import com.p2p.core.model.PeerId;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.net.*;
import java.util.Objects;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicReference;

/**
 * Coordinates NAT traversal strategies for global peer-to-peer connections.
 * Tries: direct TCP → STUN-mapped address → TCP hole punching → relay fallback.
 */
public final class NatTraversalService implements AutoCloseable {

    private static final Logger log = LoggerFactory.getLogger(NatTraversalService.class);
    private static final int CONNECT_TIMEOUT_MS = 5000;
    private static final int HOLE_PUNCH_TIMEOUT_MS = 7000;

    private final StunClient stun;
    private final RegistryClient registry;
    private final ScheduledExecutorService scheduler;
    private final AtomicReference<StunClient.StunResult> cachedStun = new AtomicReference<>();

    private volatile String localPeerId;

    public NatTraversalService(RegistryClient registry) {
        this.stun = new StunClient();
        this.registry = Objects.requireNonNull(registry);
        this.scheduler = Executors.newSingleThreadScheduledExecutor(r -> {
            Thread t = new Thread(r, "nat-traversal");
            t.setDaemon(true);
            return t;
        });
    }

    public void start(String peerId, String displayName, int tcpPort) {
        this.localPeerId = peerId;
        scheduler.submit(() -> {
            var result = stun.discover();
            cachedStun.set(result);
            if (result.success()) {
                log.info("Public address: {}:{} (NAT: {})", result.publicIp(), result.publicPort(), result.natType());
                registry.register(
                    PeerId.fromHex(peerId), displayName, tcpPort,
                    result.publicIp(), result.publicPort()
                );
                scheduleHeartbeat();
            } else {
                log.warn("STUN failed ({}), registering with private address", result.error());
                String local;
                try { local = InetAddress.getLocalHost().getHostAddress(); } catch (Exception e) { local = "0.0.0.0"; }
                registry.register(PeerId.fromHex(peerId), displayName, tcpPort, local, tcpPort);
                scheduleHeartbeat();
            }
        });
    }

    private void scheduleHeartbeat() {
        scheduler.scheduleAtFixedRate(() -> {
            if (localPeerId != null) {
                boolean ok = registry.heartbeat(PeerId.fromHex(localPeerId));
                if (!ok) log.debug("Registry heartbeat failed");
            }
        }, registry.getHeartbeatInterval(), registry.getHeartbeatInterval(), TimeUnit.SECONDS);
    }

    public ConnectResult connect(PeerInfo peer, int localTcpPort) {
        if (peer.getAddress() == null) return new ConnectResult(false, null, "No peer address");

        try {
            Socket sock = tryConnect(peer.getAddress(), peer.getPort());
            if (sock != null) {
                log.info("Direct TCP connection to {}:{} succeeded", peer.getAddress().getHostAddress(), peer.getPort());
                return new ConnectResult(true, sock, null);
            }
        } catch (Exception ignored) {}

        StunClient.StunResult stunResult = cachedStun.get();
        if (stunResult != null && stunResult.success() && stunResult.publicIp() != null) {
            try {
                InetAddress publicAddr = InetAddress.getByName(stunResult.publicIp());
                Socket sock = tryConnect(publicAddr, peer.getPort());
                if (sock != null) {
                    log.info("STUN-mapped connection to {}:{} succeeded", stunResult.publicIp(), peer.getPort());
                    return new ConnectResult(true, sock, null);
                }
                sock = tryConnect(peer.getAddress(), stunResult.publicPort());
                if (sock != null) {
                    log.info("STUN-mapped port connection to {}:{} succeeded", peer.getAddress().getHostAddress(), stunResult.publicPort());
                    return new ConnectResult(true, sock, null);
                }
            } catch (Exception ignored) {}
        }

        try {
            Socket sock = tryHolePunch(peer);
            if (sock != null) {
                log.info("TCP hole punch to {} succeeded", peer.getDisplayName());
                return new ConnectResult(true, sock, null);
            }
        } catch (Exception ignored) {}

        return new ConnectResult(false, null, "Direct, STUN, and hole punch all failed");
    }

    public StunClient.StunResult getPublicAddress() {
        return cachedStun.get();
    }

    public RegistryClient getRegistry() { return registry; }

    @Override
    public void close() {
        if (localPeerId != null) {
            registry.unregister(localPeerId);
        }
        scheduler.shutdownNow();
    }

    // --- Private ---

    private Socket tryConnect(InetAddress addr, int port) {
        try {
            Socket sock = new Socket();
            sock.setTcpNoDelay(true);
            sock.setSoTimeout(CONNECT_TIMEOUT_MS);
            sock.connect(new InetSocketAddress(addr, port), CONNECT_TIMEOUT_MS);
            sock.setSoTimeout(30000);
            return sock;
        } catch (Exception e) {
            try { Thread.sleep(100); } catch (InterruptedException ignored) {}
            return null;
        }
    }

    private Socket tryHolePunch(PeerInfo peer) {
        if (peer.getAddress() == null) return null;
        var result = new Socket[1];
        var latch = new CountDownLatch(2);
        var ex = new AtomicReference<Exception>();

        Runnable punch = () -> {
            try (ServerSocket ss = new ServerSocket(0)) {
                ss.setSoTimeout(HOLE_PUNCH_TIMEOUT_MS);
                latch.countDown();
                latch.await(HOLE_PUNCH_TIMEOUT_MS, TimeUnit.MILLISECONDS);
                try {
                    Socket s = new Socket();
                    s.setTcpNoDelay(true);
                    s.connect(new InetSocketAddress(peer.getAddress(), peer.getPort()), 2000);
                    result[0] = s;
                } catch (Exception ignored) {}
                if (result[0] == null) {
                    try { result[0] = ss.accept(); } catch (Exception ignored) {}
                }
            } catch (Exception e) {
                ex.set(e);
            }
        };

        Thread t = new Thread(punch, "hole-punch-" + peer.getDisplayName());
        t.setDaemon(true);
        t.start();
        try { t.join(HOLE_PUNCH_TIMEOUT_MS + 2000); } catch (InterruptedException ignored) {}

        return result[0];
    }

    public record ConnectResult(boolean success, Socket socket, String error) {}
}

