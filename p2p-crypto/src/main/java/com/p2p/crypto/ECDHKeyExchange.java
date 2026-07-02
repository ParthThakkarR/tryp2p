package com.p2p.crypto;

import com.p2p.core.exception.CryptoException;
import com.p2p.core.exception.ErrorCode;

import javax.crypto.KeyAgreement;
import java.security.*;
import java.security.spec.*;
import java.util.Arrays;

/**
 * ECDH (Elliptic Curve Diffie-Hellman) key exchange implementation on secp256r1 (NIST P-256).
 * Generates an ephemeral key pair and derives a shared secret from a remote peer's public key.
 * Suitable for one-shot use per session; the key pair is generated in the constructor.
 */
public final class ECDHKeyExchange {

    // --- Constants ---

    private static final String ALGORITHM = "EC";
    private static final String KEY_AGREEMENT = "ECDH";
    private static final String CURVE = "secp256r1";

    // --- Fields ---

    private final KeyPair localKeyPair;

    // --- Constructor ---

    /**
     * Generates a new ephemeral ECDH key pair on the secp256r1 curve.
     *
     * @throws CryptoException if the key pair generation fails
     */
    public ECDHKeyExchange() throws CryptoException {
        try {
            KeyPairGenerator kpg = KeyPairGenerator.getInstance(ALGORITHM);
            kpg.initialize(new ECGenParameterSpec(CURVE), new SecureRandom());
            this.localKeyPair = kpg.generateKeyPair();
        } catch (NoSuchAlgorithmException | InvalidAlgorithmParameterException e) {
            throw new CryptoException(ErrorCode.HANDSHAKE_FAILED,
                    "Failed to generate ECDH key pair on " + CURVE, e);
        }
    }

    // --- Public API ---

    /**
     * Returns the local public key encoded in X.509 format for transmission to the remote peer.
     *
     * @return X.509-encoded public key bytes
     */
    public byte[] getPublicKeyBytes() {
        return localKeyPair.getPublic().getEncoded();
    }

    /**
     * Derives a shared secret from the remote peer's public key using ECDH.
     *
     * @param remotePublicKeyBytes the X.509-encoded public key from the remote peer
     * @return the raw shared secret bytes
     * @throws CryptoException if the public key is invalid or the key agreement fails
     */
    public byte[] deriveSharedSecret(byte[] remotePublicKeyBytes) throws CryptoException {
        if (remotePublicKeyBytes == null) {
            throw new CryptoException(ErrorCode.HANDSHAKE_FAILED,
                    "Remote public key must not be null");
        }
        try {
            KeyFactory kf = KeyFactory.getInstance(ALGORITHM);
            X509EncodedKeySpec spec = new X509EncodedKeySpec(remotePublicKeyBytes);
            PublicKey remotePublicKey = kf.generatePublic(spec);

            KeyAgreement ka = KeyAgreement.getInstance(KEY_AGREEMENT);
            ka.init(localKeyPair.getPrivate());
            ka.doPhase(remotePublicKey, true);

            return ka.generateSecret();
        } catch (NoSuchAlgorithmException | InvalidKeySpecException |
                 InvalidKeyException e) {
            throw new CryptoException(ErrorCode.HANDSHAKE_FAILED,
                    "Failed to derive shared secret from remote public key", e);
        }
    }

    // --- Utilities ---

    /**
     * Computes the SHA-256 fingerprint of a public key for identification and certificate pinning.
     * The fingerprint is formatted as colon-separated uppercase hex octets (e.g. {@code "AB:CD:EF"}).
     *
     * @param publicKeyBytes the X.509-encoded public key bytes
     * @return colon-separated SHA-256 fingerprint string
     */
    public static String computeFingerprint(byte[] publicKeyBytes) {
        if (publicKeyBytes == null) {
            throw new IllegalArgumentException("Public key bytes must not be null");
        }
        try {
            MessageDigest md = MessageDigest.getInstance("SHA-256");
            byte[] hash = md.digest(publicKeyBytes);
            StringBuilder sb = new StringBuilder(hash.length * 3 - 1);
            for (int i = 0; i < hash.length; i++) {
                if (i > 0) sb.append(':');
                sb.append(String.format("%02X", hash[i]));
            }
            return sb.toString();
        } catch (NoSuchAlgorithmException e) {
            throw new RuntimeException("SHA-256 not available", e);
        }
    }

    @Override
    public String toString() {
        return String.format("ECDHKeyExchange[algorithm=%s, curve=%s]", KEY_AGREEMENT, CURVE);
    }
}
