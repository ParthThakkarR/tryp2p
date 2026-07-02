package com.p2p.security.credential;

import com.p2p.core.util.PlatformUtils;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import java.util.concurrent.ConcurrentHashMap;

/**
 * Windows Credential Manager-based credential store.
 * Currently uses an in-memory store as a placeholder; on non-Windows
 * platforms logs a warning and degrades to the same in-memory store.
 */
public final class WindowsCredentialStore implements CredentialStore {

    // --- Constants ---

    private static final Logger log = LoggerFactory.getLogger(WindowsCredentialStore.class);

    // --- Fields ---

    private final Map<String, byte[]> store = new ConcurrentHashMap<>();

    // --- Constructor ---

    /**
     * Constructs a Windows credential store. If the current OS is not Windows,
     * a warning is logged and an in-memory fallback is used.
     */
    public WindowsCredentialStore() {
        if (PlatformUtils.getCurrentOS() != PlatformUtils.OSFamily.WINDOWS) {
            log.debug("Not on Windows, falling back to in-memory store");
        }
    }

    // --- Public API ---

    /**
     * Stores a credential under the composite key "service:key".
     *
     * @param service    the service namespace
     * @param key        the credential key
     * @param credential the raw credential bytes
     */
    @Override
    public void save(String service, String key, byte[] credential) {
        Objects.requireNonNull(service, "service must not be null");
        Objects.requireNonNull(key, "key must not be null");
        Objects.requireNonNull(credential, "credential must not be null");
        String compositeKey = service + ":" + key;
        store.put(compositeKey, credential);
        log.debug("Stored credential for {}", compositeKey);
    }

    /**
     * Loads a previously stored credential.
     *
     * @param service the service namespace
     * @param key     the credential key
     * @return an Optional containing the credential bytes, or empty if not found
     */
    @Override
    public Optional<byte[]> load(String service, String key) {
        Objects.requireNonNull(service, "service must not be null");
        Objects.requireNonNull(key, "key must not be null");
        return Optional.ofNullable(store.get(service + ":" + key));
    }

    /**
     * Deletes a stored credential.
     *
     * @param service the service namespace
     * @param key     the credential key
     * @return true if the credential existed and was deleted
     */
    @Override
    public boolean delete(String service, String key) {
        Objects.requireNonNull(service, "service must not be null");
        Objects.requireNonNull(key, "key must not be null");
        return store.remove(service + ":" + key) != null;
    }

    /**
     * Checks whether a credential exists.
     *
     * @param service the service namespace
     * @param key     the credential key
     * @return true if the credential exists
     */
    @Override
    public boolean contains(String service, String key) {
        Objects.requireNonNull(service, "service must not be null");
        Objects.requireNonNull(key, "key must not be null");
        return store.containsKey(service + ":" + key);
    }

    @Override
    public String toString() {
        return String.format("WindowsCredentialStore[storeSize=%d]", store.size());
    }
}
