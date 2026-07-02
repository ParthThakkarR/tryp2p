package com.p2p.core.exception;

/**
 * Exception for binary protocol violations such as malformed messages,
 * unsupported versions, or unexpected message types.
 */
public class ProtocolException extends P2PException {
    public ProtocolException(ErrorCode errorCode, String message) {
        super(errorCode, message);
    }

    public ProtocolException(ErrorCode errorCode, String message, Throwable cause) {
        super(errorCode, message, cause);
    }
}
