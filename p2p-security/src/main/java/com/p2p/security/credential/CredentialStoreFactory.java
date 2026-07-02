package com.p2p.security.credential;

import com.p2p.core.util.PlatformUtils;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.Objects;

/**
 * Factory that creates the platform-appropriate {@link CredentialStore} implementation.
 * On Windows the Windows Credential Manager is used, on macOS the Keychain,
 * and on Linux / unknown platforms a file-backed encrypted store.
 */
public final class CredentialStoreFactory {

    // --- Constants ---

    private static final Logger log = LoggerFactory.getLogger(CredentialStoreFactory.class);
    private static final String DEFAULT_DATA_DIR = ".p2p";

    // --- Constructor ---

    private CredentialStoreFactory() {
        // utility class
    }

    // --- Public API ---

    /**
     * Creates a credential store using the default user data directory (~/.p2p).
     *
     * @return a platform-appropriate credential store
     */
    public static CredentialStore create() {
        return create(Paths.get(System.getProperty("user.home"), DEFAULT_DATA_DIR));
    }

    /**
     * Creates a credential store with an explicit data directory (used for the
     * file-backed fallback).
     *
     * @param dataDir the directory for file-backed credential storage
     * @return a platform-appropriate credential store
     */
    public static CredentialStore create(Path dataDir) {
        Objects.requireNonNull(dataDir, "dataDir must not be null");
        try {
            CredentialStore store = switch (PlatformUtils.getCurrentOS()) {
                case WINDOWS -> new WindowsCredentialStore();
                case MACOS -> new MacCredentialStore();
                case LINUX -> new FileBackedCredentialStore(dataDir.resolve("credentials"));
                case UNKNOWN -> new FileBackedCredentialStore(dataDir.resolve("credentials"));
            };
            log.info("Created credential store: {}", store.getClass().getSimpleName());
            return store;
        } catch (Exception e) {
            log.warn("Failed to create platform credential store, using file-backed: {}",
                    e.getMessage());
            return new FileBackedCredentialStore(dataDir.resolve("credentials"));
        }
    }
}
