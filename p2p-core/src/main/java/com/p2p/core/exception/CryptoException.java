package com.p2p.core.exception;

/**
 * Exception for cryptographic operation failures such as encryption,
 * decryption, key exchange, or integrity checks.
 */
public class CryptoException extends P2PException {
    public CryptoException(ErrorCode errorCode, String message) {
        super(errorCode, message);
    }

    public CryptoException(ErrorCode errorCode, String message, Throwable cause) {
        super(errorCode, message, cause);
    }
}
