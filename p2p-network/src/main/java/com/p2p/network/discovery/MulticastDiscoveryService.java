package com.p2p.network.discovery;

import com.p2p.core.config.AppConfig;
import com.p2p.core.model.*;
import com.p2p.core.service.PeerDiscoveryService;
import com.p2p.core.util.NetworkUtils;
import com.p2p.core.util.PlatformUtils;
import com.p2p.network.protocol.*;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.net.*;
import java.nio.ByteBuffer;
import java.time.Duration;
import java.time.Instant;
import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicBoolean;

/**
 * UDP multicast-based peer discovery service for LAN environments.
 *
 * <p>Periodically announces the local peer's presence via multicast and listens for
 * announcements from other peers. Detects new peers, updates existing peer info,
 * and expires stale entries after a configurable timeout.
 *
 * <p>Joins the multicast group on all active non-loopback network interfaces
 * to support multi-homed hosts.
 */
public class MulticastDiscoveryService implements PeerDiscoveryService {

    private static final Logger log = LoggerFactory.getLogger(MulticastDiscoveryService.class);

    private final AppConfig config;
    private final PeerId localPeerId;
    private final ConcurrentMap<PeerId, PeerInfo> peers = new ConcurrentHashMap<>();
    private final List<PeerDiscoveryListener> listeners = new CopyOnWriteArrayList<>();
    private final AtomicBoolean running = new AtomicBoolean(false);

    private ScheduledExecutorService scheduler;
    private ExecutorService listenerExecutor;
    private MulticastSocket multicastSocket;
    private InetSocketAddress multicastGroup;

    /**
     * Creates a multicast discovery service.
     *
     * @param config      the application configuration (must not be null)
     * @param localPeerId the local peer identifier (must not be null)
     */
    public MulticastDiscoveryService(AppConfig config, PeerId localPeerId) {
        this.config = Objects.requireNonNull(config, "config must not be null");
        this.localPeerId = Objects.requireNonNull(localPeerId, "localPeerId must not be null");
    }

    // --- Lifecycle ---

    @Override
    public void start() {
        if (running.getAndSet(true)) {
            log.warn("Discovery service already running");
            return;
        }

        try {
            InetAddress group = InetAddress.getByName(config.getMulticastGroup());
            multicastGroup = new InetSocketAddress(group, config.getDiscoveryPort());

            multicastSocket = new MulticastSocket(config.getDiscoveryPort());
            multicastSocket.setReuseAddress(true);
            multicastSocket.setSoTimeout(1000);

            for (NetworkInterface ni : NetworkUtils.getActiveInterfaces()) {
                try {
                    multicastSocket.joinGroup(multicastGroup, ni);
                    log.info("Joined multicast group {} on interface {}",
                            config.getMulticastGroup(), ni.getDisplayName());
                } catch (IOException e) {
                    log.debug("Failed to join multicast on {}: {}",
                            ni.getDisplayName(), e.getMessage());
                }
            }

            scheduler = Executors.newScheduledThreadPool(2, r -> {
                Thread t = new Thread(r, "discovery-scheduler");
                t.setDaemon(true);
                return t;
            });
            listenerExecutor = Executors.newVirtualThreadPerTaskExecutor();

            scheduler.execute(this::listenLoop);
            scheduler.scheduleAtFixedRate(this::announcePresence, 0, 5, TimeUnit.SECONDS);
            scheduler.scheduleAtFixedRate(this::expireStalePeers, 10, 10, TimeUnit.SECONDS);

            log.info("Discovery service started on {}:{}",
                    config.getMulticastGroup(), config.getDiscoveryPort());

        } catch (IOException e) {
            running.set(false);
            log.error("Failed to start discovery service: {}", e.getMessage(), e);
        }
    }

    @Override
    public void stop() {
        if (!running.getAndSet(false)) return;

        if (scheduler != null) {
            scheduler.shutdownNow();
        }
        if (listenerExecutor != null) {
            listenerExecutor.close();
        }
        if (multicastSocket != null && !multicastSocket.isClosed()) {
            try {
                for (NetworkInterface ni : NetworkUtils.getActiveInterfaces()) {
                    try {
                        multicastSocket.leaveGroup(multicastGroup, ni);
                    } catch (IOException ignored) {
                    }
                }
            } finally {
                multicastSocket.close();
            }
        }

        log.info("Discovery service stopped");
    }

    @Override
    public boolean isRunning() {
        return running.get();
    }

    // --- Peer queries ---

    @Override
    public List<PeerInfo> getDiscoveredPeers() {
        return List.copyOf(peers.values());
    }

    @Override
    public Optional<PeerInfo> findPeer(PeerId peerId) {
        return Optional.ofNullable(peers.get(peerId));
    }

    @Override
    public Optional<PeerInfo> findPeerByName(String name) {
        if (name == null) return Optional.empty();
        String lower = name.toLowerCase();
        return peers.values().stream()
                .filter(p -> p.getDisplayName().toLowerCase().contains(lower))
                .findFirst();
    }

    // --- Announcement ---

    @Override
    public void announcePresence() {
        if (!running.get()) return;

        try {
            MessageFrame announce = Messages.discoveryAnnounce(
                    localPeerId,
                    config.getDisplayName(),
                    config.getTcpPort(),
                    PlatformUtils.getPlatformString(),
                    "1.0.0"
            );

            byte[] data = announce.toBytes();
            DatagramPacket packet = new DatagramPacket(
                    data, data.length, multicastGroup);

            multicastSocket.send(packet);
            log.trace("Sent discovery announcement");

        } catch (IOException e) {
            log.debug("Failed to send announcement: {}", e.getMessage());
        }
    }

    // --- Listeners ---

    @Override
    public void addListener(PeerDiscoveryListener listener) {
        listeners.add(listener);
    }

    @Override
    public void removeListener(PeerDiscoveryListener listener) {
        listeners.remove(listener);
    }

    // --- Internal ---

    /**
     * Listens for incoming UDP multicast packets and processes them.
     * Runs in a loop until the service is stopped.
     */
    private void listenLoop() {
        byte[] buffer = new byte[4096];
        while (running.get()) {
            try {
                DatagramPacket packet = new DatagramPacket(buffer, buffer.length);
                multicastSocket.receive(packet);

                byte[] received = Arrays.copyOf(packet.getData(), packet.getLength());
                InetAddress senderAddress = packet.getAddress();

                processPacket(received, senderAddress);

            } catch (SocketTimeoutException e) {
                // Expected — allows periodic check of running flag
            } catch (IOException e) {
                if (running.get()) {
                    log.debug("Error receiving discovery packet: {}", e.getMessage());
                }
            }
        }
    }

    /**
     * Processes a received discovery packet, extracting peer info and
     * notifying listeners.
     */
    private void processPacket(byte[] data, InetAddress senderAddress) {
        try {
            MessageFrame frame = MessageFrame.fromBytes(data);

            if (frame.getType() != MessageType.DISCOVERY_ANNOUNCE
                    && frame.getType() != MessageType.DISCOVERY_RESPONSE) {
                return;
            }

            PayloadReader reader = new PayloadReader(frame.getData());
            PeerId peerId = PeerId.fromBytes(reader.readBytes());

            if (peerId.equals(localPeerId)) return;

            String displayName = reader.readString();
            int tcpPort = reader.readInt();
            String os = reader.readString();
            String version = reader.readString();

            PeerInfo peerInfo = PeerInfo.builder()
                    .peerId(peerId)
                    .displayName(displayName)
                    .address(senderAddress)
                    .port(tcpPort)
                    .operatingSystem(os)
                    .appVersion(version)
                    .lastSeen(Instant.now())
                    .status(PeerStatus.ONLINE)
                    .build();

            PeerInfo existing = peers.put(peerId, peerInfo);
            if (existing == null) {
                log.info("Discovered new peer: {} at {}", displayName,
                        NetworkUtils.formatEndpoint(senderAddress, tcpPort));
                notifyListeners(l -> l.onPeerDiscovered(peerInfo));
            } else {
                notifyListeners(l -> l.onPeerUpdated(peerInfo));
            }

            if (frame.getType() == MessageType.DISCOVERY_ANNOUNCE) {
                sendResponse(senderAddress);
            }

        } catch (Exception e) {
            log.trace("Failed to process discovery packet: {}", e.getMessage());
        }
    }

    /**
     * Sends a discovery response back to the announcing peer.
     */
    private void sendResponse(InetAddress targetAddress) {
        try {
            MessageFrame response = Messages.discoveryResponse(
                    localPeerId, config.getDisplayName(),
                    config.getTcpPort(), PlatformUtils.getPlatformString(), "1.0.0");

            byte[] data = response.toBytes();
            DatagramPacket packet = new DatagramPacket(
                    data, data.length, multicastGroup);
            multicastSocket.send(packet);
        } catch (IOException e) {
            log.debug("Failed to send discovery response: {}", e.getMessage());
        }
    }

    /**
     * Removes peers whose last-seen timestamp exceeds the configured timeout.
     */
    private void expireStalePeers() {
        Instant cutoff = Instant.now().minus(config.getPeerTimeout());
        peers.entrySet().removeIf(entry -> {
            PeerInfo peer = entry.getValue();
            if (peer.getLastSeen().isBefore(cutoff)) {
                log.info("Peer expired: {} (last seen: {})",
                        peer.getDisplayName(), peer.getLastSeen());
                notifyListeners(l -> l.onPeerLost(peer));
                return true;
            }
            return false;
        });
    }

    /**
     * Notifies all registered listeners on virtual threads.
     */
    private void notifyListeners(java.util.function.Consumer<PeerDiscoveryListener> action) {
        for (PeerDiscoveryListener listener : listeners) {
            listenerExecutor.execute(() -> {
                try {
                    action.accept(listener);
                } catch (Exception e) {
                    log.error("Error in discovery listener: {}", e.getMessage(), e);
                }
            });
        }
    }
}
