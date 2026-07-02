package com.p2p.network.protocol;

import com.p2p.core.model.*;

/**
 * Static factory methods for creating protocol {@link MessageFrame} instances.
 *
 * <p>Each method builds the appropriate payload using {@link PayloadBuilder} and
 * wraps it in a {@link MessageFrame} with the correct {@link MessageType}.
 * This is the single place where the wire format of every message is defined.
 */
public final class Messages {

    private Messages() {}

    // --- Discovery ---

    /**
     * Creates a discovery announcement frame for LAN broadcast.
     *
     * @param peerId      the local peer ID
     * @param displayName the human-readable display name
     * @param tcpPort     the TCP port the peer is listening on
     * @param os          the operating system identifier
     * @param version     the application version string
     * @return a DISCOVERY_ANNOUNCE frame
     */
    public static MessageFrame discoveryAnnounce(PeerId peerId, String displayName,
                                                  int tcpPort, String os, String version) {
        byte[] payload = new PayloadBuilder()
                .writeBytes(peerId.toBytes())
                .writeString(displayName)
                .writeInt(tcpPort)
                .writeString(os)
                .writeString(version)
                .build();
        return new MessageFrame(MessageType.DISCOVERY_ANNOUNCE, payload);
    }

    /**
     * Creates a discovery response frame in reply to an announce.
     *
     * @param peerId      the local peer ID
     * @param displayName the human-readable display name
     * @param tcpPort     the TCP port the peer is listening on
     * @param os          the operating system identifier
     * @param version     the application version string
     * @return a DISCOVERY_RESPONSE frame
     */
    public static MessageFrame discoveryResponse(PeerId peerId, String displayName,
                                                   int tcpPort, String os, String version) {
        byte[] payload = new PayloadBuilder()
                .writeBytes(peerId.toBytes())
                .writeString(displayName)
                .writeInt(tcpPort)
                .writeString(os)
                .writeString(version)
                .build();
        return new MessageFrame(MessageType.DISCOVERY_RESPONSE, payload);
    }

    // --- Handshake ---

    /**
     * Creates a handshake init frame to start an encrypted session.
     *
     * @param peerId      the local peer ID
     * @param publicKey   the ECDH public key bytes
     * @param noncePrefix the AES-GCM nonce prefix (4 bytes)
     * @return a HANDSHAKE_INIT frame
     */
    public static MessageFrame handshakeInit(PeerId peerId, byte[] publicKey, byte[] noncePrefix) {
        byte[] payload = new PayloadBuilder()
                .writeBytes(peerId.toBytes())
                .writeBytes(noncePrefix)
                .writeBytes(publicKey)
                .build();
        return new MessageFrame(MessageType.HANDSHAKE_INIT, payload);
    }

    /**
     * Creates a handshake response frame to complete key exchange.
     *
     * @param peerId    the local peer ID
     * @param publicKey the ECDH public key bytes
     * @return a HANDSHAKE_RESPONSE frame
     */
    public static MessageFrame handshakeResponse(PeerId peerId, byte[] publicKey) {
        byte[] payload = new PayloadBuilder()
                .writeBytes(peerId.toBytes())
                .writeBytes(publicKey)
                .build();
        return new MessageFrame(MessageType.HANDSHAKE_RESPONSE, payload);
    }

    // --- Transfer ---

    /**
     * Creates a transfer request frame containing file/directory metadata.
     *
     * @param sessionId the unique transfer session identifier
     * @param metadata  the file/directory metadata
     * @return a TRANSFER_REQUEST frame
     */
    public static MessageFrame transferRequest(String sessionId, FileMetadata metadata) {
        byte[] payload = new PayloadBuilder()
                .writeString(sessionId)
                .writeString(metadata.getFileName())
                .writeString(metadata.getRelativePath())
                .writeLong(metadata.getFileSize())
                .writeString(metadata.getSha256Hash() != null ? metadata.getSha256Hash() : "")
                .writeLong(metadata.getLastModified())
                .writeBoolean(metadata.isDirectory())
                .writeBoolean(metadata.isCompressible())
                .writeLong(metadata.getTotalChunks())
                .writeLong(metadata.getChunkSize())
                .build();
        return new MessageFrame(MessageType.TRANSFER_REQUEST, payload);
    }

    /**
     * Creates a transfer accept frame acknowledging a transfer request.
     *
     * @param sessionId the transfer session identifier
     * @return a TRANSFER_ACCEPT frame
     */
    public static MessageFrame transferAccept(String sessionId) {
        byte[] payload = new PayloadBuilder()
                .writeString(sessionId)
                .build();
        return new MessageFrame(MessageType.TRANSFER_ACCEPT, payload);
    }

    /**
     * Creates a transfer reject frame refusing a transfer request.
     *
     * @param sessionId the transfer session identifier
     * @param reason    the rejection reason
     * @return a TRANSFER_REJECT frame
     */
    public static MessageFrame transferReject(String sessionId, String reason) {
        byte[] payload = new PayloadBuilder()
                .writeString(sessionId)
                .writeString(reason)
                .build();
        return new MessageFrame(MessageType.TRANSFER_REJECT, payload);
    }

    /**
     * Creates a transfer complete frame signaling successful transfer.
     *
     * @param sessionId the transfer session identifier
     * @param fileHash  the SHA-256 hash of the complete file
     * @return a TRANSFER_COMPLETE frame
     */
    public static MessageFrame transferComplete(String sessionId, String fileHash) {
        byte[] payload = new PayloadBuilder()
                .writeString(sessionId)
                .writeString(fileHash)
                .build();
        return new MessageFrame(MessageType.TRANSFER_COMPLETE, payload);
    }

    // --- Chunks ---

    /**
     * Creates a chunk data frame carrying a single file chunk.
     *
     * <p>Uses {@code long} for chunkIndex to support unlimited file sizes.
     *
     * @param sessionId      the transfer session identifier
     * @param chunkIndex     the zero-based chunk index
     * @param offset         the byte offset of this chunk in the file
     * @param originalLength the original (uncompressed) chunk length
     * @param compressed     whether the chunk data is compressed
     * @param chunkHash      the SHA-256 hash of the chunk
     * @param data           the chunk payload bytes
     * @return a CHUNK_DATA frame
     */
    public static MessageFrame chunkData(String sessionId, long chunkIndex, long offset,
                                          int originalLength, boolean compressed,
                                          String chunkHash, byte[] data) {
        byte[] payload = new PayloadBuilder()
                .writeString(sessionId)
                .writeLong(chunkIndex)
                .writeLong(offset)
                .writeInt(originalLength)
                .writeBoolean(compressed)
                .writeString(chunkHash)
                .writeBytes(data)
                .build();
        return new MessageFrame(MessageType.CHUNK_DATA, payload);
    }

    /**
     * Creates a chunk acknowledgment frame confirming receipt.
     *
     * @param sessionId  the transfer session identifier
     * @param chunkIndex the zero-based chunk index
     * @return a CHUNK_ACK frame
     */
    public static MessageFrame chunkAck(String sessionId, long chunkIndex) {
        byte[] payload = new PayloadBuilder()
                .writeString(sessionId)
                .writeLong(chunkIndex)
                .build();
        return new MessageFrame(MessageType.CHUNK_ACK, payload);
    }

    /**
     * Creates a chunk negative-acknowledgment frame indicating a problem.
     *
     * @param sessionId  the transfer session identifier
     * @param chunkIndex the zero-based chunk index
     * @param reason     the failure reason
     * @return a CHUNK_NACK frame
     */
    public static MessageFrame chunkNack(String sessionId, long chunkIndex, String reason) {
        byte[] payload = new PayloadBuilder()
                .writeString(sessionId)
                .writeLong(chunkIndex)
                .writeString(reason)
                .build();
        return new MessageFrame(MessageType.CHUNK_NACK, payload);
    }

    // --- Resume ---

    /**
     * Creates a resume request frame to query transfer state.
     *
     * @param sessionId the transfer session identifier
     * @param chunkData serialized chunk state data
     * @return a RESUME_REQUEST frame
     */
    public static MessageFrame resumeRequest(String sessionId, byte[] chunkData) {
        byte[] payload = new PayloadBuilder()
                .writeString(sessionId)
                .writeBytes(chunkData)
                .build();
        return new MessageFrame(MessageType.RESUME_REQUEST, payload);
    }

    /**
     * Creates a resume response frame with accept/reject status.
     *
     * @param sessionId the transfer session identifier
     * @param accepted  whether the resume was accepted
     * @param chunkData serialized chunk state data
     * @return a RESUME_RESPONSE frame
     */
    public static MessageFrame resumeResponse(String sessionId, boolean accepted, byte[] chunkData) {
        byte[] payload = new PayloadBuilder()
                .writeString(sessionId)
                .writeBoolean(accepted)
                .writeBytes(chunkData)
                .build();
        return new MessageFrame(MessageType.RESUME_RESPONSE, payload);
    }

    // --- Error ---

    /**
     * Creates an error notification frame.
     *
     * @param message the error description
     * @return an ERROR frame
     */
    public static MessageFrame error(String message) {
        byte[] payload = new PayloadBuilder()
                .writeString(message)
                .build();
        return new MessageFrame(MessageType.ERROR, payload);
    }
}
