package com.p2p.core.exception;

/**
 * Base exception for all P2P application errors.
 * Carries a structured {@link ErrorCode} for programmatic handling.
 */
public class P2PException extends Exception {

    private final ErrorCode errorCode;

    /**
     * Constructs a new P2PException with the given error code and message.
     *
     * @param errorCode the categorized error code
     * @param message   the detail message
     */
    public P2PException(ErrorCode errorCode, String message) {
        super(message);
        this.errorCode = errorCode;
    }

    /**
     * Constructs a new P2PException with the given error code, message, and cause.
     *
     * @param errorCode the categorized error code
     * @param message   the detail message
     * @param cause     the root cause
     */
    public P2PException(ErrorCode errorCode, String message, Throwable cause) {
        super(message, cause);
        this.errorCode = errorCode;
    }

    /**
     * Returns the categorized error code.
     */
    public ErrorCode getErrorCode() {
        return errorCode;
    }

    @Override
    public String toString() {
        return String.format("P2PException[errorCode=%s, message=%s]", errorCode, getMessage());
    }
}
