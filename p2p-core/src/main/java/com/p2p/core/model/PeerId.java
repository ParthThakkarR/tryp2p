package com.p2p.core.model;

import java.io.Serializable;
import java.security.SecureRandom;
import java.util.HexFormat;
import java.util.Objects;

/**
 * Unique identifier for a peer in the P2P network.
 * Generated from a cryptographically secure random source.
 * Immutable and thread-safe.
 */
public final class PeerId implements Serializable, Comparable<PeerId> {

    private static final long serialVersionUID = 1L;
    private static final int ID_LENGTH_BYTES = 16;
    private static final SecureRandom SECURE_RANDOM = new SecureRandom();

    private final byte[] id;
    private final String hexString;

    private PeerId(byte[] id) {
        if (id == null || id.length != ID_LENGTH_BYTES) {
            throw new IllegalArgumentException(
                    "PeerId must be exactly " + ID_LENGTH_BYTES + " bytes");
        }
        this.id = id.clone();
        this.hexString = HexFormat.of().formatHex(this.id);
    }

    // --- Factory methods ---

    /**
     * Generates a new cryptographically random PeerId.
     */
    public static PeerId generate() {
        byte[] bytes = new byte[ID_LENGTH_BYTES];
        SECURE_RANDOM.nextBytes(bytes);
        return new PeerId(bytes);
    }

    /**
     * Creates a PeerId from a hex string representation.
     *
     * @param hex 32-character hex string
     * @throws IllegalArgumentException if hex string is invalid
     */
    public static PeerId fromHex(String hex) {
        Objects.requireNonNull(hex, "Hex string must not be null");
        if (hex.length() != ID_LENGTH_BYTES * 2) {
            throw new IllegalArgumentException(
                    "Hex string must be exactly " + (ID_LENGTH_BYTES * 2) + " characters");
        }
        byte[] bytes = HexFormat.of().parseHex(hex);
        return new PeerId(bytes);
    }

    /**
     * Creates a PeerId from raw bytes.
     *
     * @param bytes 16-byte array
     * @throws IllegalArgumentException if bytes length is invalid
     */
    public static PeerId fromBytes(byte[] bytes) {
        return new PeerId(bytes);
    }

    // --- Getters ---

    /**
     * Returns a defensive copy of the raw ID bytes.
     */
    public byte[] toBytes() {
        return id.clone();
    }

    /**
     * Returns the hex string representation (32 chars, lowercase).
     */
    public String toHex() {
        return hexString;
    }

    /**
     * Returns a shortened display form (first 8 hex chars).
     */
    public String toShortString() {
        return hexString.substring(0, 8);
    }

    // --- Comparable / Object overrides ---

    @Override
    public int compareTo(PeerId other) {
        return this.hexString.compareTo(other.hexString);
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof PeerId other)) return false;
        return java.util.Arrays.equals(id, other.id);
    }

    @Override
    public int hashCode() {
        return java.util.Arrays.hashCode(id);
    }

    @Override
    public String toString() {
        return "PeerId[" + hexString + "]";
    }
}
