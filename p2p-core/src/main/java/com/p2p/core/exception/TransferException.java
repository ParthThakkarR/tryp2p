package com.p2p.core.exception;

/**
 * Exception for file transfer operation failures such as disk errors,
 * hash mismatches, or rejected transfers.
 */
public class TransferException extends P2PException {
    public TransferException(ErrorCode errorCode, String message) {
        super(errorCode, message);
    }

    public TransferException(ErrorCode errorCode, String message, Throwable cause) {
        super(errorCode, message, cause);
    }
}
