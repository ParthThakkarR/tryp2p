package com.p2p.core.config;

import java.nio.file.Path;
import java.nio.file.Paths;
import java.time.Duration;
import java.util.Objects;

/**
 * Central configuration for the P2P application. Immutable after creation.
 * CLI > env > file > defaults.
 */
public final class AppConfig {

    // --- Defaults ---
    public static final int DEFAULT_TCP_PORT = 9877;
    public static final int DEFAULT_DISCOVERY_PORT = 9876;
    public static final String DEFAULT_MULTICAST_GROUP = "239.255.80.50";
    public static final long DEFAULT_CHUNK_SIZE = 1024 * 1024; // 1 MB
    public static final int DEFAULT_PARALLELISM = 4;
    public static final int DEFAULT_MAX_PARALLELISM = 16;
    public static final Duration DEFAULT_PEER_TIMEOUT = Duration.ofSeconds(30);
    public static final Duration DEFAULT_HEARTBEAT_INTERVAL = Duration.ofSeconds(5);
    public static final int DEFAULT_MISSED_HEARTBEATS = 2;
    public static final int DEFAULT_MAX_RETRIES = 3;
    public static final double DEFAULT_DISK_SPACE_FACTOR = 1.1;
    public static final int DEFAULT_MAX_INFLIGHT_CHUNKS = 4;
    public static final Duration DEFAULT_KEY_ROTATION_INTERVAL = Duration.ofDays(7);
    public static final Duration DEFAULT_RATE_LIMIT_BLOCK_DURATION = Duration.ofMinutes(15);
    public static final int DEFAULT_DISCOVERY_RATE_LIMIT = 10;
    public static final int DEFAULT_CONNECTION_RATE_LIMIT = 5;
    public static final int DEFAULT_RESUME_EXPIRY_DAYS = 7;
    public static final int DEFAULT_AUDIT_RETENTION_DAYS = 90;
    public static final int DEFAULT_METRICS_PORT = 9090;
    public static final String DEFAULT_REGISTRY_URL = "https://registry.p2ptransfer.io";
    public static final String DEFAULT_RELAY_HOST = "relay.p2ptransfer.io";
    public static final int DEFAULT_RELAY_PORT = 9878;
    public static final boolean DEFAULT_ENABLE_GLOBAL_DISCOVERY = true;

    // --- Fields ---
    private final int tcpPort;
    private final int discoveryPort;
    private final String multicastGroup;
    private final long chunkSize;
    private final int parallelism;
    private final int maxParallelism;
    private final Duration peerTimeout;
    private final Duration heartbeatInterval;
    private final int missedHeartbeatsThreshold;
    private final int maxRetries;
    private final double diskSpaceFactor;
    private final int maxInflightChunks;
    private final Duration keyRotationInterval;
    private final Duration rateLimitBlockDuration;
    private final int discoveryRateLimit;
    private final int connectionRateLimit;
    private final int resumeExpiryDays;
    private final int auditRetentionDays;
    private final int metricsPort;
    private final Path dataDirectory;
    private final Path configFile;
    private final boolean compressionEnabled;
    private final boolean encryptionEnabled;
    private final boolean metricsEnabled;
    private final String displayName;
    private final String registryUrl;
    private final String relayHost;
    private final int relayPort;
    private final boolean enableGlobalDiscovery;

    private AppConfig(Builder builder) {
        this.tcpPort = builder.tcpPort;
        this.discoveryPort = builder.discoveryPort;
        this.multicastGroup = builder.multicastGroup;
        this.chunkSize = builder.chunkSize;
        this.parallelism = builder.parallelism;
        this.maxParallelism = builder.maxParallelism;
        this.peerTimeout = builder.peerTimeout;
        this.heartbeatInterval = builder.heartbeatInterval;
        this.missedHeartbeatsThreshold = builder.missedHeartbeatsThreshold;
        this.maxRetries = builder.maxRetries;
        this.diskSpaceFactor = builder.diskSpaceFactor;
        this.maxInflightChunks = builder.maxInflightChunks;
        this.keyRotationInterval = builder.keyRotationInterval;
        this.rateLimitBlockDuration = builder.rateLimitBlockDuration;
        this.discoveryRateLimit = builder.discoveryRateLimit;
        this.connectionRateLimit = builder.connectionRateLimit;
        this.resumeExpiryDays = builder.resumeExpiryDays;
        this.auditRetentionDays = builder.auditRetentionDays;
        this.metricsPort = builder.metricsPort;
        this.dataDirectory = builder.dataDirectory;
        this.configFile = builder.configFile;
        this.compressionEnabled = builder.compressionEnabled;
        this.encryptionEnabled = builder.encryptionEnabled;
        this.metricsEnabled = builder.metricsEnabled;
        this.displayName = builder.displayName;
        this.registryUrl = builder.registryUrl;
        this.relayHost = builder.relayHost;
        this.relayPort = builder.relayPort;
        this.enableGlobalDiscovery = builder.enableGlobalDiscovery;
    }

    // --- Getters ---

    public int getTcpPort() { return tcpPort; }
    public int getDiscoveryPort() { return discoveryPort; }
    public String getMulticastGroup() { return multicastGroup; }
    public long getChunkSize() { return chunkSize; }
    public int getParallelism() { return parallelism; }
    public int getMaxParallelism() { return maxParallelism; }
    public Duration getPeerTimeout() { return peerTimeout; }
    public Duration getHeartbeatInterval() { return heartbeatInterval; }
    public int getMissedHeartbeatsThreshold() { return missedHeartbeatsThreshold; }
    public int getMaxRetries() { return maxRetries; }
    public double getDiskSpaceFactor() { return diskSpaceFactor; }
    public int getMaxInflightChunks() { return maxInflightChunks; }
    public Duration getKeyRotationInterval() { return keyRotationInterval; }
    public Duration getRateLimitBlockDuration() { return rateLimitBlockDuration; }
    public int getDiscoveryRateLimit() { return discoveryRateLimit; }
    public int getConnectionRateLimit() { return connectionRateLimit; }
    public int getResumeExpiryDays() { return resumeExpiryDays; }
    public int getAuditRetentionDays() { return auditRetentionDays; }
    public int getMetricsPort() { return metricsPort; }
    public Path getDataDirectory() { return dataDirectory; }
    public Path getConfigFile() { return configFile; }
    public boolean isCompressionEnabled() { return compressionEnabled; }
    public boolean isEncryptionEnabled() { return encryptionEnabled; }
    public boolean isMetricsEnabled() { return metricsEnabled; }
    public String getDisplayName() { return displayName; }
    public String getRegistryUrl() { return registryUrl; }
    public String getRelayHost() { return relayHost; }
    public int getRelayPort() { return relayPort; }
    public boolean isEnableGlobalDiscovery() { return enableGlobalDiscovery; }

    /**
     * Returns the maximum bytes allowed in-flight for transfers.
     */
    public long getMaxInflightBytes() {
        return (long) maxInflightChunks * chunkSize;
    }

    @Override
    public String toString() {
        return String.format("AppConfig[tcpPort=%d, discoveryPort=%d, multicastGroup=%s, chunkSize=%d, " +
                        "parallelism=%d, maxParallelism=%d, displayName=%s]",
                tcpPort, discoveryPort, multicastGroup, chunkSize,
                parallelism, maxParallelism, displayName);
    }

    // --- Factory methods ---

    /**
     * Creates a new builder for {@link AppConfig}.
     */
    public static Builder builder() {
        return new Builder();
    }

    /**
     * Returns a config with all defaults.
     */
    public static AppConfig defaults() {
        return new Builder().build();
    }

    // --- Builder ---

    public static final class Builder {
        private int tcpPort = DEFAULT_TCP_PORT;
        private int discoveryPort = DEFAULT_DISCOVERY_PORT;
        private String multicastGroup = DEFAULT_MULTICAST_GROUP;
        private long chunkSize = DEFAULT_CHUNK_SIZE;
        private int parallelism = DEFAULT_PARALLELISM;
        private int maxParallelism = DEFAULT_MAX_PARALLELISM;
        private Duration peerTimeout = DEFAULT_PEER_TIMEOUT;
        private Duration heartbeatInterval = DEFAULT_HEARTBEAT_INTERVAL;
        private int missedHeartbeatsThreshold = DEFAULT_MISSED_HEARTBEATS;
        private int maxRetries = DEFAULT_MAX_RETRIES;
        private double diskSpaceFactor = DEFAULT_DISK_SPACE_FACTOR;
        private int maxInflightChunks = DEFAULT_MAX_INFLIGHT_CHUNKS;
        private Duration keyRotationInterval = DEFAULT_KEY_ROTATION_INTERVAL;
        private Duration rateLimitBlockDuration = DEFAULT_RATE_LIMIT_BLOCK_DURATION;
        private int discoveryRateLimit = DEFAULT_DISCOVERY_RATE_LIMIT;
        private int connectionRateLimit = DEFAULT_CONNECTION_RATE_LIMIT;
        private int resumeExpiryDays = DEFAULT_RESUME_EXPIRY_DAYS;
        private int auditRetentionDays = DEFAULT_AUDIT_RETENTION_DAYS;
        private int metricsPort = DEFAULT_METRICS_PORT;
        private Path dataDirectory = Paths.get(System.getProperty("user.home"), ".p2p");
        private Path configFile = Paths.get(System.getProperty("user.home"), ".p2p", "config.yaml");
        private boolean compressionEnabled = true;
        private boolean encryptionEnabled = true;
        private boolean metricsEnabled = false;
        private String displayName = System.getProperty("user.name", "peer");
        private String registryUrl = DEFAULT_REGISTRY_URL;
        private String relayHost = DEFAULT_RELAY_HOST;
        private int relayPort = DEFAULT_RELAY_PORT;
        private boolean enableGlobalDiscovery = DEFAULT_ENABLE_GLOBAL_DISCOVERY;

        public Builder tcpPort(int port) { this.tcpPort = port; return this; }
        public Builder discoveryPort(int port) { this.discoveryPort = port; return this; }
        public Builder multicastGroup(String group) { this.multicastGroup = group; return this; }
        public Builder chunkSize(long size) { this.chunkSize = size; return this; }
        public Builder parallelism(int p) { this.parallelism = p; return this; }
        public Builder maxParallelism(int mp) { this.maxParallelism = mp; return this; }
        public Builder peerTimeout(Duration t) { this.peerTimeout = t; return this; }
        public Builder heartbeatInterval(Duration i) { this.heartbeatInterval = i; return this; }
        public Builder missedHeartbeatsThreshold(int n) { this.missedHeartbeatsThreshold = n; return this; }
        public Builder maxRetries(int r) { this.maxRetries = r; return this; }
        public Builder diskSpaceFactor(double f) { this.diskSpaceFactor = f; return this; }
        public Builder maxInflightChunks(int n) { this.maxInflightChunks = n; return this; }
        public Builder keyRotationInterval(Duration d) { this.keyRotationInterval = d; return this; }
        public Builder rateLimitBlockDuration(Duration d) { this.rateLimitBlockDuration = d; return this; }
        public Builder discoveryRateLimit(int r) { this.discoveryRateLimit = r; return this; }
        public Builder connectionRateLimit(int r) { this.connectionRateLimit = r; return this; }
        public Builder resumeExpiryDays(int d) { this.resumeExpiryDays = d; return this; }
        public Builder auditRetentionDays(int d) { this.auditRetentionDays = d; return this; }
        public Builder metricsPort(int port) { this.metricsPort = port; return this; }
        public Builder dataDirectory(Path dir) { this.dataDirectory = dir; return this; }
        public Builder configFile(Path file) { this.configFile = file; return this; }
        public Builder compressionEnabled(boolean e) { this.compressionEnabled = e; return this; }
        public Builder encryptionEnabled(boolean e) { this.encryptionEnabled = e; return this; }
        public Builder metricsEnabled(boolean e) { this.metricsEnabled = e; return this; }
        public Builder displayName(String name) { this.displayName = name; return this; }
        public Builder registryUrl(String url) { this.registryUrl = url; return this; }
        public Builder relayHost(String host) { this.relayHost = host; return this; }
        public Builder relayPort(int port) { this.relayPort = port; return this; }
        public Builder enableGlobalDiscovery(boolean e) { this.enableGlobalDiscovery = e; return this; }

        public AppConfig build() {
            return new AppConfig(this);
        }
    }
}
