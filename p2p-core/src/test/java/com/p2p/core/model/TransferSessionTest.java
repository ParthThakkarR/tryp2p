package com.p2p.core.model;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.DisplayName;

import static org.junit.jupiter.api.Assertions.*;

@DisplayName("TransferSession Tests")
class TransferSessionTest {

    private TransferSession createTestSession() {
        FileMetadata meta = FileMetadata.builder()
                .fileName("test.bin")
                .relativePath("test.bin")
                .fileSize(4 * 1024 * 1024)
                .chunkSize(1024 * 1024)
                .build();

        return TransferSession.builder()
                .localPeerId(PeerId.generate())
                .remotePeerId(PeerId.generate())
                .fileMetadata(meta)
                .direction(TransferDirection.SEND)
                .build();
    }

    @Test
    @DisplayName("initial state is PENDING")
    void initialState() {
        TransferSession session = createTestSession();
        assertEquals(TransferState.PENDING, session.getState());
        assertEquals(0, session.getBytesTransferred());
        assertEquals(0, session.getChunksCompleted());
        assertNull(session.getStartedAt());
    }

    @Test
    @DisplayName("valid state transitions work")
    void validTransitions() {
        TransferSession session = createTestSession();
        session.transitionTo(TransferState.HANDSHAKING);
        assertEquals(TransferState.HANDSHAKING, session.getState());

        session.transitionTo(TransferState.NEGOTIATING);
        session.transitionTo(TransferState.TRANSFERRING);
        assertEquals(TransferState.TRANSFERRING, session.getState());
        assertNotNull(session.getStartedAt());
    }

    @Test
    @DisplayName("invalid transitions throw IllegalStateException")
    void invalidTransitions() {
        TransferSession session = createTestSession();
        assertThrows(IllegalStateException.class,
                () -> session.transitionTo(TransferState.TRANSFERRING));
    }

    @Test
    @DisplayName("cannot transition from terminal state")
    void cannotTransitionFromTerminal() {
        TransferSession session = createTestSession();
        session.transitionTo(TransferState.FAILED);
        assertThrows(IllegalStateException.class,
                () -> session.transitionTo(TransferState.HANDSHAKING));
    }

    @Test
    @DisplayName("FAILED and CANCELLED always reachable from non-terminal")
    void failedAlwaysReachable() {
        TransferSession session = createTestSession();
        session.transitionTo(TransferState.HANDSHAKING);
        session.transitionTo(TransferState.NEGOTIATING);
        assertDoesNotThrow(() -> session.transitionTo(TransferState.FAILED));
        assertTrue(session.getState().isTerminal());
        assertNotNull(session.getCompletedAt());
    }

    @Test
    @DisplayName("markChunkCompleted updates progress")
    void markChunkCompleted() {
        TransferSession session = createTestSession();
        session.markChunkCompleted(0, 1024 * 1024);
        assertEquals(1, session.getChunksCompleted());
        assertEquals(1024 * 1024, session.getBytesTransferred());

        session.markChunkCompleted(2, 1024 * 1024);
        assertEquals(2, session.getChunksCompleted());
    }

    @Test
    @DisplayName("duplicate chunk completion is idempotent")
    void duplicateChunkIdempotent() {
        TransferSession session = createTestSession();
        session.markChunkCompleted(0, 1024 * 1024);
        session.markChunkCompleted(0, 1024 * 1024);
        assertEquals(1, session.getChunksCompleted());
        assertEquals(1024 * 1024, session.getBytesTransferred());
    }

    @Test
    @DisplayName("markChunkCompleted rejects negative index")
    void rejectsNegativeChunk() {
        TransferSession session = createTestSession();
        assertThrows(IndexOutOfBoundsException.class,
                () -> session.markChunkCompleted(-1, 1024));
    }

    @Test
    @DisplayName("getProgress() returns accurate snapshot")
    void progressSnapshot() {
        TransferSession session = createTestSession();
        session.transitionTo(TransferState.HANDSHAKING);
        session.transitionTo(TransferState.NEGOTIATING);
        session.transitionTo(TransferState.TRANSFERRING);

        session.markChunkCompleted(0, 1024 * 1024);
        session.markChunkCompleted(1, 1024 * 1024);

        TransferProgress progress = session.getProgress();
        assertEquals(2, progress.chunksCompleted());
        assertEquals(4, progress.totalChunks());
        assertEquals(50.0, progress.percentComplete(), 0.01);
        assertEquals(2 * 1024 * 1024, progress.bytesTransferred());
    }

    @Test
    @DisplayName("chunk tracking works correctly")
    void chunkTracking() {
        TransferSession session = createTestSession();
        assertFalse(session.isChunkCompleted(0));
        session.markChunkCompleted(0, 1024 * 1024);
        assertTrue(session.isChunkCompleted(0));
        assertFalse(session.isChunkCompleted(1));

        var chunks = session.getCompletedChunksSet();
        assertTrue(chunks.contains(0L));
        assertEquals(1, chunks.size());
    }

    @Test
    @DisplayName("fail() sets error message and terminal state")
    void failSetsError() {
        TransferSession session = createTestSession();
        session.fail("Disk full");
        assertEquals(TransferState.FAILED, session.getState());
        assertEquals("Disk full", session.getErrorMessage());
        assertTrue(session.getState().isTerminal());
    }
}
