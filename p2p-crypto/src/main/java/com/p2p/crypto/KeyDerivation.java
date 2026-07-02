package com.p2p.crypto;

import com.p2p.core.exception.CryptoException;
import com.p2p.core.exception.ErrorCode;

import javax.crypto.*;
import javax.crypto.spec.*;
import java.nio.ByteBuffer;
import java.security.*;
import java.util.Arrays;

/**
 * Key derivation utilities using HKDF (HMAC-based Key Derivation Function) as defined in RFC 5869.
 * Derives AES-256 session keys from ECDH shared secrets, generates random nonce prefixes,
 * and builds per-chunk GCM nonces.
 * <p>
 * All methods are stateless and thread-safe.
 */
public final class KeyDerivation {

    // --- Constants ---

    private static final String HMAC_ALGO = "HmacSHA256";
    private static final int AES_KEY_LENGTH = 32;
    private static final int NONCE_PREFIX_LENGTH = 4;

    private KeyDerivation() {
        throw new AssertionError("Utility class — do not instantiate");
    }

    // --- Derivation ---

    /**
     * Derives an AES-256 key from an ECDH shared secret using HKDF-Expand.
     * The extract step uses a zero-valued salt (32 bytes) as recommended by RFC 5869
     * when the input key material has sufficient entropy.
     *
     * @param sharedSecret the raw ECDH shared secret bytes
     * @param info         context information (e.g. {@code "p2p-transfer-encryption"})
     * @return a 256-bit AES {@link SecretKey}
     * @throws CryptoException if key derivation fails
     */
    public static SecretKey deriveAesKey(byte[] sharedSecret, String info) throws CryptoException {
        if (sharedSecret == null) {
            throw new CryptoException(ErrorCode.ENCRYPTION_FAILED,
                    "Shared secret must not be null");
        }
        if (sharedSecret.length == 0) {
            throw new CryptoException(ErrorCode.ENCRYPTION_FAILED,
                    "Shared secret must not be empty");
        }
        if (info == null) {
            throw new CryptoException(ErrorCode.ENCRYPTION_FAILED,
                    "Context info must not be null");
        }
        try {
            byte[] prk = hkdfExtract(new byte[32], sharedSecret);
            byte[] okm = hkdfExpand(prk, info.getBytes(), AES_KEY_LENGTH);
            return new SecretKeySpec(okm, "AES");
        } catch (Exception e) {
            throw new CryptoException(ErrorCode.ENCRYPTION_FAILED,
                    "Failed to derive AES key from shared secret", e);
        }
    }

    /**
     * Generates a random 4-byte nonce prefix for use within a single session.
     * The prefix is combined with an 8-byte chunk counter to form the 12-byte GCM nonce.
     *
     * @return a 4-byte random prefix
     */
    public static byte[] generateNoncePrefix() {
        byte[] prefix = new byte[NONCE_PREFIX_LENGTH];
        new SecureRandom().nextBytes(prefix);
        return prefix;
    }

    /**
     * Builds a 12-byte GCM nonce from a 4-byte session prefix and an 8-byte chunk counter.
     * The counter is written in big-endian byte order. This guarantees unique nonces
     * for up to 2^64 chunks per session as long as the prefix is session-unique.
     *
     * @param prefix  4-byte session-unique nonce prefix
     * @param counter non-negative chunk counter
     * @return a 12-byte nonce suitable for AES-GCM
     * @throws IllegalArgumentException if prefix is null, not exactly 4 bytes, or counter is negative
     */
    public static byte[] buildNonce(byte[] prefix, long counter) {
        if (prefix == null) {
            throw new IllegalArgumentException("Nonce prefix must not be null");
        }
        if (prefix.length != 4) {
            throw new IllegalArgumentException(
                    "Nonce prefix must be exactly 4 bytes, got: " + prefix.length);
        }
        if (counter < 0) {
            throw new IllegalArgumentException(
                    "Chunk counter must be non-negative, got: " + counter);
        }
        ByteBuffer buffer = ByteBuffer.allocate(12);
        buffer.put(prefix, 0, 4);
        buffer.putLong(counter);
        return buffer.array();
    }

    // --- HKDF Implementation ---

    /**
     * HKDF-Extract: applies HMAC-SHA256 with the given salt to the input key material.
     */
    private static byte[] hkdfExtract(byte[] salt, byte[] inputKeyMaterial) throws Exception {
        Mac mac = Mac.getInstance(HMAC_ALGO);
        mac.init(new SecretKeySpec(salt, HMAC_ALGO));
        return mac.doFinal(inputKeyMaterial);
    }

    /**
     * HKDF-Expand: generates {@code outputLength} bytes of key material using HMAC-SHA256.
     * Produces up to 255 * 32 = 8160 bytes.
     */
    private static byte[] hkdfExpand(byte[] prk, byte[] info, int outputLength) throws Exception {
        Mac mac = Mac.getInstance(HMAC_ALGO);
        mac.init(new SecretKeySpec(prk, HMAC_ALGO));

        int hashLen = mac.getMacLength();
        int iterations = (int) Math.ceil((double) outputLength / hashLen);
        if (iterations > 255) {
            throw new IllegalArgumentException(
                    "Output length " + outputLength + " exceeds HKDF-Expand maximum (255 * " + hashLen + ")");
        }

        byte[] result = new byte[outputLength];
        byte[] previous = new byte[0];
        int offset = 0;

        for (int i = 1; i <= iterations; i++) {
            mac.update(previous);
            mac.update(info);
            mac.update((byte) i);
            previous = mac.doFinal();

            int toCopy = Math.min(previous.length, outputLength - offset);
            System.arraycopy(previous, 0, result, offset, toCopy);
            offset += toCopy;
        }

        return result;
    }
}
