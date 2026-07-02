package com.p2p.security.credential;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import javax.crypto.Cipher;
import javax.crypto.SecretKeyFactory;
import javax.crypto.spec.IvParameterSpec;
import javax.crypto.spec.PBEKeySpec;
import javax.crypto.spec.SecretKeySpec;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.SecureRandom;
import java.security.spec.KeySpec;
import java.util.Objects;
import java.util.Optional;

/**
 * A file-backed credential store that encrypts credentials on disk using
 * AES-256-CBC with a key derived via PBKDF2-HMAC-SHA256. Each credential
 * is stored as a separate encrypted file within the configured directory.
 */
public final class FileBackedCredentialStore implements CredentialStore {

    // --- Constants ---

    private static final Logger log = LoggerFactory.getLogger(FileBackedCredentialStore.class);
    private static final int SALT_LENGTH = 16;
    private static final int IV_LENGTH = 16;
    private static final int KEY_LENGTH = 256;
    private static final int ITERATION_COUNT = 100000;
    private static final String CIPHER_TRANSFORMATION = "AES/CBC/PKCS5Padding";
    private static final String KEY_DERIVATION_ALGORITHM = "PBKDF2WithHmacSHA256";
    private static final String CREDENTIAL_EXTENSION = ".cred";
    private static final String DEFAULT_PASSWORD = "p2p-default-password";

    // --- Fields ---

    private final Path storeDir;
    private byte[] salt;
    private byte[] iv;

    // --- Constructor ---

    /**
     * Constructs a file-backed credential store in the given directory.
     * Creates the directory and initializes salt/IV files if they do not exist.
     *
     * @param storeDir directory where encrypted credential files are stored
     */
    public FileBackedCredentialStore(Path storeDir) {
        this.storeDir = Objects.requireNonNull(storeDir, "storeDir must not be null");
        try {
            Files.createDirectories(storeDir);
            this.salt = loadOrCreateFile(storeDir.resolve("salt"), SALT_LENGTH);
            this.iv = loadOrCreateFile(storeDir.resolve("iv"), IV_LENGTH);
        } catch (Exception e) {
            throw new RuntimeException("Failed to initialize credential store", e);
        }
    }

    // --- Public API ---

    /**
     * Encrypts and stores a credential for the given service and key.
     *
     * @param service    the service namespace
     * @param key        the credential key
     * @param credential the raw credential bytes to encrypt and store
     * @throws SecurityException if encryption or file I/O fails
     */
    @Override
    public void save(String service, String key, byte[] credential) {
        Objects.requireNonNull(service, "service must not be null");
        Objects.requireNonNull(key, "key must not be null");
        Objects.requireNonNull(credential, "credential must not be null");
        try {
            SecretKeySpec keySpec = deriveKey(DEFAULT_PASSWORD.toCharArray());
            Cipher cipher = Cipher.getInstance(CIPHER_TRANSFORMATION);
            cipher.init(Cipher.ENCRYPT_MODE, keySpec, new IvParameterSpec(iv));
            byte[] encrypted = cipher.doFinal(credential);

            Path credFile = getCredFile(service, key);
            Files.write(credFile, encrypted);
            log.debug("Saved credential for {}:{}", service, key);
        } catch (Exception e) {
            throw new SecurityException("Failed to save credential", e);
        }
    }

    /**
     * Loads and decrypts a stored credential.
     *
     * @param service the service namespace
     * @param key     the credential key
     * @return an Optional containing the decrypted credential bytes, or empty if not found
     */
    @Override
    public Optional<byte[]> load(String service, String key) {
        Objects.requireNonNull(service, "service must not be null");
        Objects.requireNonNull(key, "key must not be null");
        try {
            Path credFile = getCredFile(service, key);
            if (!Files.exists(credFile)) return Optional.empty();

            byte[] encrypted = Files.readAllBytes(credFile);
            SecretKeySpec keySpec = deriveKey(DEFAULT_PASSWORD.toCharArray());
            Cipher cipher = Cipher.getInstance(CIPHER_TRANSFORMATION);
            cipher.init(Cipher.DECRYPT_MODE, keySpec, new IvParameterSpec(iv));
            return Optional.of(cipher.doFinal(encrypted));
        } catch (Exception e) {
            log.warn("Failed to load credential {}:{}", service, key);
            return Optional.empty();
        }
    }

    /**
     * Deletes a stored credential file.
     *
     * @param service the service namespace
     * @param key     the credential key
     * @return true if the file existed and was deleted, false otherwise
     */
    @Override
    public boolean delete(String service, String key) {
        Objects.requireNonNull(service, "service must not be null");
        Objects.requireNonNull(key, "key must not be null");
        try {
            Path credFile = getCredFile(service, key);
            return Files.deleteIfExists(credFile);
        } catch (Exception e) {
            return false;
        }
    }

    /**
     * Checks whether a credential file exists for the given service and key.
     *
     * @param service the service namespace
     * @param key     the credential key
     * @return true if the credential file exists
     */
    @Override
    public boolean contains(String service, String key) {
        Objects.requireNonNull(service, "service must not be null");
        Objects.requireNonNull(key, "key must not be null");
        return Files.exists(getCredFile(service, key));
    }

    @Override
    public String toString() {
        return String.format("FileBackedCredentialStore[storeDir=%s]", storeDir);
    }

    // --- Internal ---

    private Path getCredFile(String service, String key) {
        String safeName = (service + "-" + key).replaceAll("[^a-zA-Z0-9._-]", "_");
        return storeDir.resolve(safeName + CREDENTIAL_EXTENSION);
    }

    private SecretKeySpec deriveKey(char[] password) throws Exception {
        SecretKeyFactory factory = SecretKeyFactory.getInstance(KEY_DERIVATION_ALGORITHM);
        KeySpec spec = new PBEKeySpec(password, salt, ITERATION_COUNT, KEY_LENGTH);
        byte[] keyBytes = factory.generateSecret(spec).getEncoded();
        return new SecretKeySpec(keyBytes, "AES");
    }

    private byte[] loadOrCreateFile(Path path, int length) throws Exception {
        if (Files.exists(path)) {
            return Files.readAllBytes(path);
        }
        byte[] data = new byte[length];
        new SecureRandom().nextBytes(data);
        Files.write(path, data);
        return data;
    }
}
