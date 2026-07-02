package com.p2p.core.model;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.DisplayName;

import static org.junit.jupiter.api.Assertions.*;

@DisplayName("TransferState Tests")
class TransferStateTest {

    @Test
    @DisplayName("terminal states identified correctly")
    void terminalStates() {
        assertTrue(TransferState.COMPLETED.isTerminal());
        assertTrue(TransferState.FAILED.isTerminal());
        assertTrue(TransferState.CANCELLED.isTerminal());
        assertFalse(TransferState.PENDING.isTerminal());
        assertFalse(TransferState.TRANSFERRING.isTerminal());
    }

    @Test
    @DisplayName("active states identified correctly")
    void activeStates() {
        assertTrue(TransferState.HANDSHAKING.isActive());
        assertTrue(TransferState.NEGOTIATING.isActive());
        assertTrue(TransferState.TRANSFERRING.isActive());
        assertTrue(TransferState.VERIFYING.isActive());
        assertFalse(TransferState.PENDING.isActive());
        assertFalse(TransferState.PAUSED.isActive());
    }

    @Test
    @DisplayName("resumable states identified correctly")
    void resumableStates() {
        assertTrue(TransferState.PAUSED.isResumable());
        assertTrue(TransferState.INTERRUPTED.isResumable());
        assertFalse(TransferState.TRANSFERRING.isResumable());
        assertFalse(TransferState.COMPLETED.isResumable());
    }
}
