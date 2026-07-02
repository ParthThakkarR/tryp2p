package com.p2p.core.service;

import com.p2p.core.model.PeerInfo;
import com.p2p.core.model.PeerId;

import java.util.List;
import java.util.Optional;

/**
 * Service interface for discovering peers on the local network via multicast.
 */
public interface PeerDiscoveryService {

    /**
     * Starts the discovery service (begins broadcasting and listening).
     */
    void start();

    /**
     * Stops the discovery service.
     */
    void stop();

    /**
     * Returns all currently known peers.
     *
     * @return list of discovered peers
     */
    List<PeerInfo> getDiscoveredPeers();

    /**
     * Finds a peer by its ID.
     *
     * @param peerId the peer ID to search for
     * @return an Optional containing the peer, or empty if not found
     */
    Optional<PeerInfo> findPeer(PeerId peerId);

    /**
     * Finds a peer by display name (case-insensitive partial match).
     *
     * @param name the display name to search for
     * @return an Optional containing the peer, or empty if not found
     */
    Optional<PeerInfo> findPeerByName(String name);

    /**
     * Forces an immediate discovery broadcast.
     */
    void announcePresence();

    /**
     * Registers a listener for peer discovery events.
     *
     * @param listener the listener to register
     */
    void addListener(PeerDiscoveryListener listener);

    /**
     * Removes a peer discovery listener.
     *
     * @param listener the listener to remove
     */
    void removeListener(PeerDiscoveryListener listener);

    /**
     * Returns true if the discovery service is running.
     *
     * @return true if running
     */
    boolean isRunning();

    /**
     * Listener for peer discovery lifecycle events.
     */
    interface PeerDiscoveryListener {
        /** Called when a new peer is discovered. */
        void onPeerDiscovered(PeerInfo peer);

        /** Called when a known peer's info is updated. */
        void onPeerUpdated(PeerInfo peer);

        /** Called when a peer goes offline or is removed. */
        void onPeerLost(PeerInfo peer);
    }
}
