package com.p2p.core.exception;

/**
 * Exception for network-related failures such as connection timeouts,
 * unreachable peers, or multicast errors.
 */
public class NetworkException extends P2PException {
    public NetworkException(ErrorCode errorCode, String message) {
        super(errorCode, message);
    }

    public NetworkException(ErrorCode errorCode, String message, Throwable cause) {
        super(errorCode, message, cause);
    }
}
