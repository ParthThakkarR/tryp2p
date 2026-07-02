package com.p2p.core.model;

import java.net.InetAddress;
import java.time.Instant;
import java.util.Objects;

/**
 * Information about a discovered peer on the network.
 * Immutable and thread-safe.
 */
public final class PeerInfo {

    private final PeerId peerId;
    private final String displayName;
    private final InetAddress address;
    private final int port;
    private final String operatingSystem;
    private final String appVersion;
    private final Instant lastSeen;
    private final PeerStatus status;

    private PeerInfo(Builder builder) {
        this.peerId = Objects.requireNonNull(builder.peerId, "peerId must not be null");
        this.displayName = Objects.requireNonNull(builder.displayName, "displayName must not be null");
        this.address = Objects.requireNonNull(builder.address, "address must not be null");
        this.port = builder.port;
        this.operatingSystem = Objects.requireNonNull(builder.operatingSystem, "operatingSystem must not be null");
        this.appVersion = Objects.requireNonNull(builder.appVersion, "appVersion must not be null");
        this.lastSeen = Objects.requireNonNull(builder.lastSeen, "lastSeen must not be null");
        this.status = Objects.requireNonNull(builder.status, "status must not be null");

        if (port < 1 || port > 65535) {
            throw new IllegalArgumentException("Port must be between 1 and 65535, got: " + port);
        }
    }

    // --- Getters ---

    public PeerId getPeerId() { return peerId; }
    public String getDisplayName() { return displayName; }
    public InetAddress getAddress() { return address; }
    public int getPort() { return port; }
    public String getOperatingSystem() { return operatingSystem; }
    public String getAppVersion() { return appVersion; }
    public Instant getLastSeen() { return lastSeen; }
    public PeerStatus getStatus() { return status; }

    // --- Immutable operations ---

    /**
     * Returns a new PeerInfo with the lastSeen updated to now and status set to ONLINE.
     */
    public PeerInfo withRefreshed() {
        return new Builder(this)
                .lastSeen(Instant.now())
                .status(PeerStatus.ONLINE)
                .build();
    }

    /**
     * Returns a new PeerInfo with the given status.
     *
     * @param newStatus the new status for the peer
     */
    public PeerInfo withStatus(PeerStatus newStatus) {
        return new Builder(this).status(newStatus).build();
    }

    // --- Object overrides ---

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof PeerInfo other)) return false;
        return peerId.equals(other.peerId);
    }

    @Override
    public int hashCode() {
        return peerId.hashCode();
    }

    @Override
    public String toString() {
        return String.format("PeerInfo[id=%s, name=%s, addr=%s:%d, os=%s, status=%s]",
                peerId.toShortString(), displayName, address.getHostAddress(), port,
                operatingSystem, status);
    }

    public static Builder builder() {
        return new Builder();
    }

    public static final class Builder {
        private PeerId peerId;
        private String displayName;
        private InetAddress address;
        private int port;
        private String operatingSystem;
        private String appVersion;
        private Instant lastSeen = Instant.now();
        private PeerStatus status = PeerStatus.ONLINE;

        private Builder() {}

        private Builder(PeerInfo source) {
            this.peerId = source.peerId;
            this.displayName = source.displayName;
            this.address = source.address;
            this.port = source.port;
            this.operatingSystem = source.operatingSystem;
            this.appVersion = source.appVersion;
            this.lastSeen = source.lastSeen;
            this.status = source.status;
        }

        public Builder peerId(PeerId peerId) { this.peerId = peerId; return this; }
        public Builder displayName(String displayName) { this.displayName = displayName; return this; }
        public Builder address(InetAddress address) { this.address = address; return this; }
        public Builder port(int port) { this.port = port; return this; }
        public Builder operatingSystem(String os) { this.operatingSystem = os; return this; }
        public Builder appVersion(String version) { this.appVersion = version; return this; }
        public Builder lastSeen(Instant lastSeen) { this.lastSeen = lastSeen; return this; }
        public Builder status(PeerStatus status) { this.status = status; return this; }

        public PeerInfo build() {
            return new PeerInfo(this);
        }
    }
}
