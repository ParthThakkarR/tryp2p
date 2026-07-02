package com.p2p.core.service;

import com.p2p.core.exception.CryptoException;

/**
 * Service interface for per-chunk encryption and decryption operations.
 * Uses AEAD with unique per-chunk nonces derived from chunk index.
 */
public interface EncryptionService {

    /**
     * Encrypts a chunk of data using a session key.
     *
     * @param plaintext  the data to encrypt
     * @param chunkIndex the chunk index (used for nonce derivation)
     * @return encrypted data with authentication tag appended
     * @throws CryptoException if encryption fails
     */
    byte[] encryptChunk(byte[] plaintext, long chunkIndex) throws CryptoException;

    /**
     * Decrypts a chunk of data and verifies its authentication tag.
     *
     * @param ciphertext the encrypted data with authentication tag
     * @param chunkIndex the chunk index (used for nonce derivation)
     * @return decrypted plaintext
     * @throws CryptoException if decryption or authentication fails
     */
    byte[] decryptChunk(byte[] ciphertext, long chunkIndex) throws CryptoException;

    /**
     * Returns true if this service has been initialized with a session key.
     *
     * @return true if initialized
     */
    boolean isInitialized();

    /**
     * Clears all key material from memory.
     */
    void destroy();
}
