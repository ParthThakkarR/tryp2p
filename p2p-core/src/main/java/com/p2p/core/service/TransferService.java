package com.p2p.core.service;

import com.p2p.core.exception.TransferException;
import com.p2p.core.model.PeerInfo;
import com.p2p.core.model.TransferProgress;
import com.p2p.core.model.TransferSession;
import com.p2p.core.model.TransferState;

import java.nio.file.Path;
import java.util.List;

/**
 * Core service interface for managing file transfers between peers.
 * Supports send, receive, resume, pause, and cancel operations.
 */
public interface TransferService {

    /**
     * Sends a file or directory to a remote peer.
     *
     * @param filePath   path to the file or directory to send
     * @param remotePeer the target peer
     * @param priority   if true, bumps this transfer to front of queue
     * @return the created transfer session
     * @throws TransferException if the transfer cannot be initiated
     */
    TransferSession send(Path filePath, PeerInfo remotePeer, boolean priority) throws TransferException;

    /**
     * Starts listening for incoming transfer requests.
     *
     * @param saveDirectory directory to save received files
     * @param autoAccept    if true, automatically accept all incoming transfers
     */
    void startReceiving(Path saveDirectory, boolean autoAccept);

    /**
     * Stops listening for incoming transfers.
     */
    void stopReceiving();

    /**
     * Resumes an interrupted transfer session.
     *
     * @param sessionId the session to resume
     * @return the resumed session
     * @throws TransferException if the session cannot be resumed
     */
    TransferSession resume(String sessionId) throws TransferException;

    /**
     * Cancels an active transfer.
     *
     * @param sessionId the session ID to cancel
     */
    void cancel(String sessionId);

    /**
     * Pauses an active transfer.
     *
     * @param sessionId the session ID to pause
     */
    void pause(String sessionId);

    /**
     * Returns all active transfer sessions.
     *
     * @return list of active sessions
     */
    List<TransferSession> getActiveSessions();

    /**
     * Returns the transfer session with the given ID, if it exists.
     *
     * @param sessionId the session ID
     * @return the session, or null if not found
     */
    TransferSession getSession(String sessionId);

    /**
     * Registers a listener for transfer lifecycle events.
     *
     * @param listener the listener to register
     */
    void addListener(TransferListener listener);

    /**
     * Removes a transfer listener.
     *
     * @param listener the listener to remove
     */
    void removeListener(TransferListener listener);

    /**
     * Listener for transfer lifecycle events.
     */
    interface TransferListener {
        /** Called when a new transfer request is received. Returns true to accept. */
        default boolean onTransferRequested(TransferSession session) { return true; }

        /** Called when transfer state changes. */
        default void onStateChanged(TransferSession session, TransferState oldState, TransferState newState) {}

        /** Called on progress updates. */
        default void onProgress(TransferProgress progress) {}

        /** Called when transfer completes successfully. */
        default void onCompleted(TransferSession session) {}

        /** Called when transfer fails. */
        default void onFailed(TransferSession session, String errorMessage) {}
    }
}
