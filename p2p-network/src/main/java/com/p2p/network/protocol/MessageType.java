package com.p2p.network.protocol;

/**
 * Message types in the P2P binary protocol.
 *
 * <p>Each type has a unique byte identifier for wire serialization.
 * Ranges: Discovery (0x01-0x0F), Handshake (0x10-0x1F), Transfer Control
 * (0x20-0x2F), Chunk Data (0x30-0x3F), Keepalive (0x40-0x4F), Resume
 * (0x50-0x5F), Error (0xFF).
 */
public enum MessageType {

    // --- Discovery ---

    /** Periodic LAN announcement advertising local presence. */
    DISCOVERY_ANNOUNCE((byte) 0x01),

    /** Response to a discovery announcement from a peer. */
    DISCOVERY_RESPONSE((byte) 0x02),

    // --- Handshake ---

    /** Initiates an encrypted channel handshake. */
    HANDSHAKE_INIT((byte) 0x10),

    /** Response to a handshake init, completing key exchange. */
    HANDSHAKE_RESPONSE((byte) 0x11),

    // --- Transfer Control ---

    /** Requests a file/directory transfer. */
    TRANSFER_REQUEST((byte) 0x20),

    /** Accepts an incoming transfer request. */
    TRANSFER_ACCEPT((byte) 0x21),

    /** Rejects an incoming transfer request with a reason. */
    TRANSFER_REJECT((byte) 0x22),

    /** Signals that a transfer has completed successfully. */
    TRANSFER_COMPLETE((byte) 0x23),

    /** Cancels an active transfer. */
    TRANSFER_CANCEL((byte) 0x24),

    // --- Chunk Data ---

    /** Carries a single chunk of file data. */
    CHUNK_DATA((byte) 0x30),

    /** Acknowledges successful receipt of a chunk. */
    CHUNK_ACK((byte) 0x31),

    /** Negative acknowledgment indicating chunk delivery failure. */
    CHUNK_NACK((byte) 0x32),

    // --- Keepalive ---

    /** Keepalive ping to verify peer connectivity. */
    HEARTBEAT((byte) 0x40),

    /** Keepalive pong acknowledging a heartbeat. */
    HEARTBEAT_ACK((byte) 0x41),

    // --- Resume ---

    /** Requests transfer resume state from a peer. */
    RESUME_REQUEST((byte) 0x50),

    /** Response to a resume request with accepted/rejected status. */
    RESUME_RESPONSE((byte) 0x51),

    // --- Error ---

    /** Generic error notification with a message string. */
    ERROR((byte) 0xFF);

    private final byte id;

    MessageType(byte id) {
        this.id = id;
    }

    // --- Getters ---

    /**
     * Returns the wire byte identifier for this message type.
     *
     * @return the byte ID
     */
    public byte getId() {
        return id;
    }

    /**
     * Resolves a {@link MessageType} from its wire byte ID.
     *
     * @param id the byte ID to resolve
     * @return the matching message type
     * @throws IllegalArgumentException if the byte does not map to a known type
     */
    public static MessageType fromId(byte id) {
        for (MessageType type : values()) {
            if (type.id == id) {
                return type;
            }
        }
        throw new IllegalArgumentException(
                String.format("Unknown message type: 0x%02X", id));
    }
}
