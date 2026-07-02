package com.p2p.security.pinning;

import com.p2p.core.exception.CryptoException;
import com.p2p.core.model.PeerId;
import com.p2p.crypto.KeyRotationManager;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.time.Instant;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import java.util.Properties;
import java.util.concurrent.ConcurrentHashMap;

/**
 * Trust-on-first-use (TOFU) certificate pinning manager.
 * Pins the first public key seen for each peer and rejects subsequent
 * keys unless they match the pinned fingerprint or are within the
 * configured key rotation window.
 */
public final class CertificatePinningManager {

    // --- Constants ---

    private static final Logger log = LoggerFactory.getLogger(CertificatePinningManager.class);
    private static final String HASH_ALGORITHM = "SHA-256";
    private static final String PIN_FILE_NAME = "pinned-keys.properties";

    // --- Fields ---

    private final Map<PeerId, String> pinnedFingerprints = new ConcurrentHashMap<>();
    private final Map<PeerId, Instant> pinnedTimestamps = new ConcurrentHashMap<>();
    private final Path configDir;
    private final KeyRotationManager keyRotationManager;

    // --- Constructor ---

    /**
     * Constructs a pinning manager backed by the given config directory.
     *
     * @param configDir          directory for persistent pinned-key storage
     * @param keyRotationManager manager for checking rotated-key validity, may be null
     */
    public CertificatePinningManager(Path configDir, KeyRotationManager keyRotationManager) {
        this.configDir = Objects.requireNonNull(configDir, "configDir must not be null");
        this.keyRotationManager = keyRotationManager;
        loadPinnedKeys();
    }

    // --- Public API ---

    /**
     * Verifies whether a peer's public key is trusted.
     * On first contact the key is pinned automatically (TOFU).
     * On subsequent contacts the key fingerprint must match the pinned value
     * or be within the rotation window.
     *
     * @param peerId         the peer to verify
     * @param publicKeyBytes the peer's raw public key bytes
     * @return true if the key is trusted
     * @throws SecurityException if fingerprint computation fails
     */
    public boolean isTrusted(PeerId peerId, byte[] publicKeyBytes) {
        Objects.requireNonNull(peerId, "peerId must not be null");
        Objects.requireNonNull(publicKeyBytes, "publicKeyBytes must not be null");
        String fingerprint = computeFingerprint(publicKeyBytes);

        if (!pinnedFingerprints.containsKey(peerId)) {
            log.debug("First contact with peer {}, pinning key: {}",
                    peerId.toShortString(), fingerprint);
            pinKey(peerId, publicKeyBytes);
            return true;
        }

        String pinnedFingerprint = pinnedFingerprints.get(peerId);
        if (pinnedFingerprint.equals(fingerprint)) {
            return true;
        }

        if (keyRotationManager != null) {
            try {
                if (keyRotationManager.isKeyValid(publicKeyBytes)) {
                    log.info("Key accepted from rotation window for peer {}", peerId.toShortString());
                    return true;
                }
            } catch (CryptoException e) {
                log.warn("Failed to check key rotation for peer {}: {}", peerId.toShortString(), e.getMessage());
            }
        }

        log.error("KEY MISMATCH for peer {}! Expected: {}, Got: {}",
                peerId.toShortString(), pinnedFingerprint, fingerprint);
        return false;
    }

    /**
     * Pins a peer's public key fingerprint to the local store.
     *
     * @param peerId         the peer to pin
     * @param publicKeyBytes the peer's raw public key bytes
     */
    public void pinKey(PeerId peerId, byte[] publicKeyBytes) {
        Objects.requireNonNull(peerId, "peerId must not be null");
        Objects.requireNonNull(publicKeyBytes, "publicKeyBytes must not be null");
        String fingerprint = computeFingerprint(publicKeyBytes);
        pinnedFingerprints.put(peerId, fingerprint);
        pinnedTimestamps.put(peerId, Instant.now());
        savePinnedKeys();
        log.info("Pinned key for peer {}: {}", peerId.toShortString(), fingerprint);
    }

    /**
     * Returns whether a pinned key exists for the given peer.
     *
     * @param peerId the peer to check
     * @return true if a key has been pinned for this peer
     */
    public boolean hasPinnedKey(PeerId peerId) {
        Objects.requireNonNull(peerId, "peerId must not be null");
        return pinnedFingerprints.containsKey(peerId);
    }

    /**
     * Returns the pinned SHA-256 fingerprint for a peer, if one exists.
     *
     * @param peerId the peer to look up
     * @return an Optional containing the fingerprint string
     */
    public Optional<String> getPinnedFingerprint(PeerId peerId) {
        Objects.requireNonNull(peerId, "peerId must not be null");
        return Optional.ofNullable(pinnedFingerprints.get(peerId));
    }

    /**
     * Removes a pinned key for the given peer.
     *
     * @param peerId the peer to remove
     */
    public void removePinnedKey(PeerId peerId) {
        Objects.requireNonNull(peerId, "peerId must not be null");
        pinnedFingerprints.remove(peerId);
        pinnedTimestamps.remove(peerId);
        savePinnedKeys();
        log.info("Removed pinned key for peer {}", peerId.toShortString());
    }

    /**
     * Returns a snapshot list of all currently pinned peers.
     *
     * @return list of pinned peer records
     */
    public List<PinnedPeer> listPinnedPeers() {
        List<PinnedPeer> peers = new ArrayList<>();
        for (Map.Entry<PeerId, String> entry : pinnedFingerprints.entrySet()) {
            peers.add(new PinnedPeer(entry.getKey(), entry.getValue(),
                    pinnedTimestamps.get(entry.getKey())));
        }
        return peers;
    }

    // --- Static utilities ---

    /**
     * Computes the colon-delimited hex SHA-256 fingerprint of public key bytes.
     *
     * @param publicKeyBytes the raw public key bytes
     * @return the fingerprint string (e.g. "AB:CD:EF:...")
     */
    public static String computeFingerprint(byte[] publicKeyBytes) {
        Objects.requireNonNull(publicKeyBytes, "publicKeyBytes must not be null");
        try {
            MessageDigest md = MessageDigest.getInstance(HASH_ALGORITHM);
            byte[] hash = md.digest(publicKeyBytes);
            StringBuilder sb = new StringBuilder();
            for (int i = 0; i < hash.length; i++) {
                if (i > 0) sb.append(':');
                sb.append(String.format("%02X", hash[i]));
            }
            return sb.toString();
        } catch (Exception e) {
            throw new RuntimeException("SHA-256 not available", e);
        }
    }

    // --- Internal ---

    private void loadPinnedKeys() {
        try {
            Path pinFile = configDir.resolve(PIN_FILE_NAME);
            if (Files.exists(pinFile)) {
                Properties props = new Properties();
                try (var is = Files.newInputStream(pinFile)) {
                    props.load(is);
                }
                for (String key : props.stringPropertyNames()) {
                    try {
                        PeerId peerId = PeerId.fromHex(key);
                        pinnedFingerprints.put(peerId, props.getProperty(key));
                    } catch (Exception e) {
                        log.debug("Skipping invalid pinned key entry: {}", key);
                    }
                }
                log.info("Loaded {} pinned keys", pinnedFingerprints.size());
            }
        } catch (Exception e) {
            log.warn("Failed to load pinned keys: {}", e.getMessage());
        }
    }

    private void savePinnedKeys() {
        try {
            Path pinFile = configDir.resolve(PIN_FILE_NAME);
            Files.createDirectories(configDir);
            Properties props = new Properties();
            for (Map.Entry<PeerId, String> entry : pinnedFingerprints.entrySet()) {
                props.setProperty(entry.getKey().toString(), entry.getValue());
            }
            try (var os = Files.newOutputStream(pinFile)) {
                props.store(os, "P2P Pinned Peer Keys");
            }
        } catch (Exception e) {
            log.warn("Failed to save pinned keys: {}", e.getMessage());
        }
    }

    @Override
    public String toString() {
        return String.format("CertificatePinningManager[pinned=%d, configDir=%s]",
                pinnedFingerprints.size(), configDir);
    }

    // --- Value types ---

    /**
     * A record representing a pinned peer with its fingerprint and pin timestamp.
     */
    public record PinnedPeer(PeerId peerId, String fingerprint, Instant pinnedAt) {}
}
