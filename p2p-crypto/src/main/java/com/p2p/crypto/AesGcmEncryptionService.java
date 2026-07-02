package com.p2p.crypto;

import com.p2p.core.exception.CryptoException;
import com.p2p.core.exception.ErrorCode;
import com.p2p.core.service.EncryptionService;

import javax.crypto.*;
import javax.crypto.spec.*;
import java.security.*;
import java.util.Arrays;

/**
 * AES-256-GCM encryption service for chunked file transfer.
 * Each chunk receives a unique 12-byte nonce built from a session-unique 4-byte prefix
 * and an 8-byte chunk counter, ensuring nonce uniqueness across all chunks in a session.
 * An authentication tag (128-bit) is appended to each ciphertext for tamper detection.
 * <p>
 * Thread-safe once initialized; supports one-time use via {@link #destroy()}.
 */
public final class AesGcmEncryptionService implements EncryptionService {

    // --- Constants ---

    private static final String ALGORITHM = "AES/GCM/NoPadding";
    private static final int GCM_TAG_LENGTH_BITS = 128;
    private static final int GCM_NONCE_LENGTH = 12;

    // --- Fields ---

    private final SecretKey key;
    private final byte[] noncePrefix;
    private volatile boolean destroyed;

    // --- Constructor ---

    /**
     * Creates an encryption service with a derived session key and nonce prefix.
     *
     * @param key         the AES-256 key (must be 32 bytes)
     * @param noncePrefix 4-byte session-unique nonce prefix
     */
    public AesGcmEncryptionService(SecretKey key, byte[] noncePrefix) {
        if (key == null) {
            throw new IllegalArgumentException("AES key must not be null");
        }
        if (key.getEncoded().length != 32) {
            throw new IllegalArgumentException("AES-256 key required (32 bytes), got: "
                    + key.getEncoded().length + " bytes");
        }
        if (noncePrefix == null) {
            throw new IllegalArgumentException("Nonce prefix must not be null");
        }
        if (noncePrefix.length != 4) {
            throw new IllegalArgumentException("Nonce prefix must be exactly 4 bytes, got: "
                    + noncePrefix.length + " bytes");
        }
        this.key = key;
        this.noncePrefix = noncePrefix.clone();
    }

    // --- Encryption ---

    /**
     * Encrypts a single chunk of data using AES-256-GCM.
     * The nonce is derived from the session prefix and the chunk index.
     *
     * @param plaintext  the plaintext chunk data
     * @param chunkIndex zero-based chunk index within the file
     * @return ciphertext with appended 128-bit GCM authentication tag
     * @throws CryptoException if encryption fails
     */
    @Override
    public byte[] encryptChunk(byte[] plaintext, long chunkIndex) throws CryptoException {
        checkNotDestroyed();
        try {
            byte[] nonce = KeyDerivation.buildNonce(noncePrefix, chunkIndex);

            Cipher cipher = Cipher.getInstance(ALGORITHM);
            GCMParameterSpec gcmSpec = new GCMParameterSpec(GCM_TAG_LENGTH_BITS, nonce);
            cipher.init(Cipher.ENCRYPT_MODE, key, gcmSpec);

            return cipher.doFinal(plaintext);
        } catch (GeneralSecurityException e) {
            throw new CryptoException(ErrorCode.ENCRYPTION_FAILED,
                    "Failed to encrypt chunk " + chunkIndex, e);
        }
    }

    /**
     * Decrypts a single chunk of data previously encrypted with this service.
     * Verifies the GCM authentication tag; throws on integrity violation.
     *
     * @param ciphertext the ciphertext with appended 128-bit GCM authentication tag
     * @param chunkIndex zero-based chunk index within the file
     * @return decrypted plaintext chunk data
     * @throws CryptoException if decryption fails or the data has been tampered with
     */
    @Override
    public byte[] decryptChunk(byte[] ciphertext, long chunkIndex) throws CryptoException {
        checkNotDestroyed();
        try {
            byte[] nonce = KeyDerivation.buildNonce(noncePrefix, chunkIndex);

            Cipher cipher = Cipher.getInstance(ALGORITHM);
            GCMParameterSpec gcmSpec = new GCMParameterSpec(GCM_TAG_LENGTH_BITS, nonce);
            cipher.init(Cipher.DECRYPT_MODE, key, gcmSpec);

            return cipher.doFinal(ciphertext);
        } catch (AEADBadTagException e) {
            throw new CryptoException(ErrorCode.INTEGRITY_VIOLATION,
                    "Chunk " + chunkIndex + " authentication failed — data may have been tampered", e);
        } catch (GeneralSecurityException e) {
            throw new CryptoException(ErrorCode.DECRYPTION_FAILED,
                    "Failed to decrypt chunk " + chunkIndex, e);
        }
    }

    // --- Lifecycle ---

    @Override
    public boolean isInitialized() {
        return !destroyed;
    }

    @Override
    public void destroy() {
        destroyed = true;
    }

    // --- Internal ---

    private void checkNotDestroyed() throws CryptoException {
        if (destroyed) {
            throw new CryptoException(ErrorCode.ENCRYPTION_FAILED,
                    "Encryption service has been destroyed");
        }
    }

    @Override
    public String toString() {
        return String.format("AesGcmEncryptionService[initialized=%s]", isInitialized());
    }
}
