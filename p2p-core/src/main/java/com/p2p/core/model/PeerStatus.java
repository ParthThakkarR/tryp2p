package com.p2p.core.model;

/**
 * Represents the connection status of a peer.
 */
public enum PeerStatus {
    /** Peer is online and reachable. */
    ONLINE,

    /** Peer has been discovered but not yet verified. */
    DISCOVERED,

    /** Peer has missed heartbeats and may be unreachable. */
    UNREACHABLE,

    /** Peer is confirmed offline. */
    OFFLINE,

    /** Peer has been blocked due to security policy. */
    BLOCKED
}
