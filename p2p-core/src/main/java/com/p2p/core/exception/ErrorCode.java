package com.p2p.core.exception;

/**
 * Categorized error codes for structured error handling and reporting.
 * Each code has a numeric value and a default human-readable message.
 */
public enum ErrorCode {
    // --- Network Errors (1xxx) ---
    /** Network is unreachable. */
    NETWORK_UNREACHABLE(1001, "Network is unreachable"),
    /** Connection refused by peer. */
    CONNECTION_REFUSED(1002, "Connection refused by peer"),
    /** Connection timed out. */
    CONNECTION_TIMEOUT(1003, "Connection timed out"),
    /** Connection lost during transfer. */
    CONNECTION_LOST(1004, "Connection lost during transfer"),
    /** Multicast discovery failed. */
    MULTICAST_FAILED(1005, "Multicast discovery failed"),
    /** Failed to bind to port. */
    BIND_FAILED(1006, "Failed to bind to port"),

    // --- Crypto Errors (2xxx) ---
    /** ECDH handshake failed. */
    HANDSHAKE_FAILED(2001, "ECDH handshake failed"),
    /** Encryption failed. */
    ENCRYPTION_FAILED(2002, "Encryption failed"),
    /** Decryption failed. */
    DECRYPTION_FAILED(2003, "Decryption failed"),
    /** Data integrity check failed. */
    INTEGRITY_VIOLATION(2004, "Data integrity check failed"),
    /** Key rotation failed. */
    KEY_ROTATION_FAILED(2005, "Key rotation failed"),
    /** Peer certificate does not match pinned fingerprint. */
    CERTIFICATE_MISMATCH(2006, "Peer certificate does not match pinned fingerprint"),

    // --- Transfer Errors (3xxx) ---
    /** Source file not found. */
    FILE_NOT_FOUND(3001, "Source file not found"),
    /** Failed to read source file. */
    FILE_READ_ERROR(3002, "Failed to read source file"),
    /** Failed to write destination file. */
    FILE_WRITE_ERROR(3003, "Failed to write destination file"),
    /** Insufficient disk space on receiver. */
    INSUFFICIENT_DISK_SPACE(3004, "Insufficient disk space on receiver"),
    /** Transfer rejected by receiver. */
    TRANSFER_REJECTED(3005, "Transfer rejected by receiver"),
    /** Transfer cancelled by user. */
    TRANSFER_CANCELLED(3006, "Transfer cancelled by user"),
    /** Failed to resume transfer. */
    RESUME_FAILED(3007, "Failed to resume transfer"),
    /** Chunk SHA-256 hash mismatch. */
    CHUNK_HASH_MISMATCH(3008, "Chunk SHA-256 hash mismatch"),
    /** File SHA-256 hash mismatch. */
    FILE_HASH_MISMATCH(3009, "File SHA-256 hash mismatch"),
    /** Maximum retry attempts exceeded. */
    MAX_RETRIES_EXCEEDED(3010, "Maximum retry attempts exceeded"),
    /** Transfer failed. */
    TRANSFER_FAILED(3011, "Transfer failed"),

    // --- Protocol Errors (4xxx) ---
    /** Invalid or malformed message. */
    INVALID_MESSAGE(4001, "Invalid or malformed message"),
    /** Unsupported protocol version. */
    UNSUPPORTED_VERSION(4002, "Unsupported protocol version"),
    /** Message exceeds maximum size. */
    MESSAGE_TOO_LARGE(4003, "Message exceeds maximum size"),
    /** Unexpected message type in current state. */
    UNEXPECTED_MESSAGE(4004, "Unexpected message type in current state"),

    // --- Security Errors (5xxx) ---
    /** Peer is blocked due to rate limiting. */
    PEER_BLOCKED(5001, "Peer is blocked due to rate limiting"),
    /** Rate limit exceeded. */
    RATE_LIMIT_EXCEEDED(5002, "Rate limit exceeded"),
    /** Peer authentication failed. */
    AUTHENTICATION_FAILED(5003, "Peer authentication failed"),
    /** Unauthorized operation. */
    UNAUTHORIZED(5004, "Unauthorized operation"),

    // --- System Errors (9xxx) ---
    /** Internal error. */
    INTERNAL_ERROR(9001, "Internal error"),
    /** Configuration error. */
    CONFIGURATION_ERROR(9002, "Configuration error"),
    /** Unsupported platform. */
    UNSUPPORTED_PLATFORM(9003, "Unsupported platform");

    private final int code;
    private final String defaultMessage;

    ErrorCode(int code, String defaultMessage) {
        this.code = code;
        this.defaultMessage = defaultMessage;
    }

    /** Returns the numeric error code. */
    public int getCode() { return code; }

    /** Returns the default human-readable message. */
    public String getDefaultMessage() { return defaultMessage; }

    /**
     * Returns true if this error is potentially recoverable via retry.
     */
    public boolean isRetryable() {
        return switch (this) {
            case CONNECTION_TIMEOUT, CONNECTION_LOST, NETWORK_UNREACHABLE,
                 MULTICAST_FAILED, CHUNK_HASH_MISMATCH -> true;
            default -> false;
        };
    }
}
