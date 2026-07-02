package com.p2p.core.model;

/**
 * Lifecycle states of a file transfer session.
 */
public enum TransferState {
    /** Transfer session created but not started. */
    PENDING,

    /** Performing ECDH key exchange handshake. */
    HANDSHAKING,

    /** Key exchange complete, negotiating transfer parameters. */
    NEGOTIATING,

    /** Actively transferring data. */
    TRANSFERRING,

    /** Transfer paused by user or system. */
    PAUSED,

    /** Transfer interrupted — eligible for resume. */
    INTERRUPTED,

    /** Performing final integrity verification. */
    VERIFYING,

    /** Transfer completed successfully. */
    COMPLETED,

    /** Transfer failed with an error. */
    FAILED,

    /** Transfer cancelled by user. */
    CANCELLED;

    /**
     * Returns true if the transfer is in a terminal state.
     */
    public boolean isTerminal() {
        return this == COMPLETED || this == FAILED || this == CANCELLED;
    }

    /**
     * Returns true if the transfer is actively processing.
     */
    public boolean isActive() {
        return this == HANDSHAKING || this == NEGOTIATING
                || this == TRANSFERRING || this == VERIFYING;
    }

    /**
     * Returns true if the transfer can be resumed.
     */
    public boolean isResumable() {
        return this == PAUSED || this == INTERRUPTED;
    }
}
