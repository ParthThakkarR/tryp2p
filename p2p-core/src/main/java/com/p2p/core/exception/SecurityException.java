package com.p2p.core.exception;

/**
 * Exception for security policy violations such as rate limiting,
 * authentication failures, or blocked peers.
 */
public class SecurityException extends P2PException {
    public SecurityException(ErrorCode errorCode, String message) {
        super(errorCode, message);
    }

    public SecurityException(ErrorCode errorCode, String message, Throwable cause) {
        super(errorCode, message, cause);
    }
}
