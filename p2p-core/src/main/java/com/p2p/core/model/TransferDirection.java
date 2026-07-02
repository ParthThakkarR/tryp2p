package com.p2p.core.model;

/**
 * Direction of a file transfer from the local peer's perspective.
 */
public enum TransferDirection {
    /** Local peer is sending data. */
    SEND,

    /** Local peer is receiving data. */
    RECEIVE
}
