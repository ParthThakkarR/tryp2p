package com.p2p.security.credential;

import java.util.Optional;

/**
 * Abstraction for securely persisting and retrieving credentials
 * (e.g. private keys, authentication tokens) keyed by service and key name.
 * Implementations may use platform-native secure storage (Windows Credential
 * Manager, macOS Keychain) or a file-backed fallback.
 */
public interface CredentialStore {

    /**
     * Stores a credential for the given service and key.
     *
     * @param service    the service namespace (e.g. "p2p")
     * @param key        the credential key within the service
     * @param credential the raw credential bytes to store
     * @throws SecurityException if storage fails
     */
    void save(String service, String key, byte[] credential) throws SecurityException;

    /**
     * Loads a previously stored credential.
     *
     * @param service the service namespace
     * @param key     the credential key
     * @return an Optional containing the credential bytes, or empty if not found
     * @throws SecurityException if retrieval fails
     */
    Optional<byte[]> load(String service, String key) throws SecurityException;

    /**
     * Deletes a stored credential.
     *
     * @param service the service namespace
     * @param key     the credential key
     * @return true if the credential existed and was deleted, false otherwise
     * @throws SecurityException if deletion fails
     */
    boolean delete(String service, String key) throws SecurityException;

    /**
     * Checks whether a credential exists for the given service and key.
     *
     * @param service the service namespace
     * @param key     the credential key
     * @return true if the credential exists
     * @throws SecurityException if the check fails
     */
    boolean contains(String service, String key) throws SecurityException;

    /**
     * Exception thrown when a credential operation fails.
     */
    class SecurityException extends RuntimeException {
        public SecurityException(String message) { super(message); }
        public SecurityException(String message, Throwable cause) { super(message, cause); }
    }
}
