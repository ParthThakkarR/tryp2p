package com.p2p.crypto;

import com.p2p.core.exception.CryptoException;
import com.p2p.core.exception.ErrorCode;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.BeforeEach;

import javax.crypto.SecretKey;
import java.util.Arrays;

import static org.junit.jupiter.api.Assertions.*;

@DisplayName("Crypto Layer Tests")
class CryptoTest {

    private ECDHKeyExchange alice;
    private ECDHKeyExchange bob;

    @BeforeEach
    void setUp() throws Exception {
        alice = new ECDHKeyExchange();
        bob = new ECDHKeyExchange();
    }

    @Test
    @DisplayName("ECDH key exchange derives identical shared secrets")
    void ecdhSharedSecret() throws Exception {
        byte[] aliceSecret = alice.deriveSharedSecret(bob.getPublicKeyBytes());
        byte[] bobSecret = bob.deriveSharedSecret(alice.getPublicKeyBytes());

        assertArrayEquals(aliceSecret, bobSecret);
        assertTrue(aliceSecret.length > 0);
    }

    @Test
    @DisplayName("Different key pairs produce different shared secrets")
    void differentKeyPairsDifferentSecrets() throws Exception {
        ECDHKeyExchange charlie = new ECDHKeyExchange();
        byte[] aliceBob = alice.deriveSharedSecret(bob.getPublicKeyBytes());
        byte[] aliceCharlie = alice.deriveSharedSecret(charlie.getPublicKeyBytes());

        assertFalse(Arrays.equals(aliceBob, aliceCharlie));
    }

    @Test
    @DisplayName("Public key fingerprint is consistent")
    void fingerprint() {
        String fp1 = ECDHKeyExchange.computeFingerprint(alice.getPublicKeyBytes());
        String fp2 = ECDHKeyExchange.computeFingerprint(alice.getPublicKeyBytes());
        assertEquals(fp1, fp2);

        // Different keys produce different fingerprints
        String fp3 = ECDHKeyExchange.computeFingerprint(bob.getPublicKeyBytes());
        assertNotEquals(fp1, fp3);

        // Fingerprint format: XX:XX:XX:...
        assertTrue(fp1.contains(":"));
        assertEquals(95, fp1.length()); // 32 bytes * 3 - 1 = 95 chars
    }

    @Test
    @DisplayName("AES-256-GCM encrypt/decrypt round-trip")
    void aesGcmRoundTrip() throws Exception {
        byte[] sharedSecret = alice.deriveSharedSecret(bob.getPublicKeyBytes());
        SecretKey key = KeyDerivation.deriveAesKey(sharedSecret, "test");
        byte[] noncePrefix = KeyDerivation.generateNoncePrefix();

        AesGcmEncryptionService encryptor = new AesGcmEncryptionService(key, noncePrefix);
        AesGcmEncryptionService decryptor = new AesGcmEncryptionService(key, noncePrefix);

        byte[] plaintext = "Hello, P2P World! This is a test of AES-256-GCM encryption.".getBytes();
        byte[] encrypted = encryptor.encryptChunk(plaintext, 0);
        byte[] decrypted = decryptor.decryptChunk(encrypted, 0);

        assertArrayEquals(plaintext, decrypted);
        assertFalse(Arrays.equals(plaintext, encrypted)); // Must be different
    }

    @Test
    @DisplayName("AES-GCM detects tampering")
    void aesGcmTamperDetection() throws Exception {
        byte[] sharedSecret = alice.deriveSharedSecret(bob.getPublicKeyBytes());
        SecretKey key = KeyDerivation.deriveAesKey(sharedSecret, "test");
        byte[] noncePrefix = KeyDerivation.generateNoncePrefix();

        AesGcmEncryptionService service = new AesGcmEncryptionService(key, noncePrefix);

        byte[] plaintext = "sensitive data".getBytes();
        byte[] encrypted = service.encryptChunk(plaintext, 0);

        // Tamper with ciphertext
        encrypted[5] ^= 0xFF;

        CryptoException ex = assertThrows(CryptoException.class,
                () -> service.decryptChunk(encrypted, 0));
        assertEquals(ErrorCode.INTEGRITY_VIOLATION, ex.getErrorCode());
    }

    @Test
    @DisplayName("Different chunk indices produce different ciphertexts")
    void differentNoncesPerChunk() throws Exception {
        byte[] sharedSecret = alice.deriveSharedSecret(bob.getPublicKeyBytes());
        SecretKey key = KeyDerivation.deriveAesKey(sharedSecret, "test");
        byte[] noncePrefix = KeyDerivation.generateNoncePrefix();

        AesGcmEncryptionService service = new AesGcmEncryptionService(key, noncePrefix);

        byte[] plaintext = "same data".getBytes();
        byte[] enc0 = service.encryptChunk(plaintext, 0);
        byte[] enc1 = service.encryptChunk(plaintext, 1);

        assertFalse(Arrays.equals(enc0, enc1));
    }

    @Test
    @DisplayName("Wrong chunk index fails decryption")
    void wrongChunkIndex() throws Exception {
        byte[] sharedSecret = alice.deriveSharedSecret(bob.getPublicKeyBytes());
        SecretKey key = KeyDerivation.deriveAesKey(sharedSecret, "test");
        byte[] noncePrefix = KeyDerivation.generateNoncePrefix();

        AesGcmEncryptionService service = new AesGcmEncryptionService(key, noncePrefix);

        byte[] encrypted = service.encryptChunk("data".getBytes(), 0);

        // Decrypt with wrong index
        assertThrows(CryptoException.class,
                () -> service.decryptChunk(encrypted, 1));
    }

    @Test
    @DisplayName("Nonce construction produces 12-byte nonces")
    void nonceConstruction() {
        byte[] prefix = {0x01, 0x02, 0x03, 0x04};
        byte[] nonce = KeyDerivation.buildNonce(prefix, 42L);

        assertEquals(12, nonce.length);
        assertEquals(0x01, nonce[0]);
        assertEquals(0x04, nonce[3]);
    }

    @Test
    @DisplayName("Destroyed service rejects operations")
    void destroyedServiceRejects() throws Exception {
        byte[] sharedSecret = alice.deriveSharedSecret(bob.getPublicKeyBytes());
        SecretKey key = KeyDerivation.deriveAesKey(sharedSecret, "test");
        byte[] noncePrefix = KeyDerivation.generateNoncePrefix();

        AesGcmEncryptionService service = new AesGcmEncryptionService(key, noncePrefix);
        assertTrue(service.isInitialized());

        service.destroy();
        assertFalse(service.isInitialized());
        assertThrows(CryptoException.class,
                () -> service.encryptChunk("data".getBytes(), 0));
    }

    @Test
    @DisplayName("Large chunk encryption/decryption")
    void largeChunk() throws Exception {
        byte[] sharedSecret = alice.deriveSharedSecret(bob.getPublicKeyBytes());
        SecretKey key = KeyDerivation.deriveAesKey(sharedSecret, "test");
        byte[] noncePrefix = KeyDerivation.generateNoncePrefix();

        AesGcmEncryptionService service = new AesGcmEncryptionService(key, noncePrefix);

        byte[] large = new byte[1024 * 1024]; // 1 MB
        Arrays.fill(large, (byte) 0xAB);

        byte[] encrypted = service.encryptChunk(large, 0);
        byte[] decrypted = service.decryptChunk(encrypted, 0);

        assertArrayEquals(large, decrypted);
    }
}
