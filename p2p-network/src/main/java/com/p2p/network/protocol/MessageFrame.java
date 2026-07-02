package com.p2p.network.protocol;

import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Objects;

/**
 * Represents a framed message in the P2P binary protocol.
 *
 * <p>Wire format:
 * <pre>
 * ┌─────────┬──────────┬───────────┬──────────────────┐
 * │ Magic   │ Version  │ Length    │ Payload          │
 * │ (4B)    │ (1B)     │ (4B)     │ (variable)       │
 * │ "P2PF"  │ 0x01     │ uint32   │ Type(1B) + Data  │
 * └─────────┴──────────┴───────────┴──────────────────┘
 * </pre>
 *
 * <p>Total header: 9 bytes. Payload starts with 1-byte MessageType.
 * Maximum payload size: 16 MB.
 */
public final class MessageFrame {

    // --- Constants ---

    public static final byte[] MAGIC = {'P', '2', 'P', 'F'};
    public static final byte PROTOCOL_VERSION = 0x01;
    public static final int HEADER_SIZE = 9;
    public static final int MAX_PAYLOAD_SIZE = 16 * 1024 * 1024;
    public static final int MIN_PAYLOAD_SIZE = 1;

    private final MessageType type;
    private final byte[] data;

    /**
     * Creates a new message frame.
     *
     * @param type the message type (must not be null)
     * @param data the payload data excluding the type byte; may be null/empty
     * @throws IllegalArgumentException if the total payload (type byte + data) exceeds
     *         {@link #MAX_PAYLOAD_SIZE}
     */
    public MessageFrame(MessageType type, byte[] data) {
        this.type = Objects.requireNonNull(type, "type must not be null");
        this.data = data != null ? data.clone() : new byte[0];

        int payloadSize = 1 + this.data.length;
        if (payloadSize > MAX_PAYLOAD_SIZE) {
            throw new IllegalArgumentException(
                    "Payload size " + payloadSize + " exceeds maximum " + MAX_PAYLOAD_SIZE);
        }
    }

    // --- Getters ---

    /**
     * Returns the message type.
     *
     * @return the message type
     */
    public MessageType getType() {
        return type;
    }

    /**
     * Returns a defensive copy of the payload data (excluding the type byte).
     *
     * @return a new byte array containing the payload data
     */
    public byte[] getData() {
        return data.clone();
    }

    /**
     * Returns the length of the payload data in bytes (excluding the type byte).
     *
     * @return the data length
     */
    public int getDataLength() {
        return data.length;
    }

    /**
     * Returns the payload data decoded as a UTF-8 string.
     *
     * @return the decoded string
     */
    public String getDataAsString() {
        return new String(data, StandardCharsets.UTF_8);
    }

    /**
     * Returns the total wire size of this frame in bytes (header + type + data).
     *
     * @return the wire size
     */
    public int getWireSize() {
        return HEADER_SIZE + 1 + data.length;
    }

    // --- Serialization ---

    /**
     * Serializes this frame to a byte array for wire transmission.
     *
     * <p>Format: {@code [MAGIC(4)][VERSION(1)][LENGTH(4)][TYPE(1)][DATA(N)]}
     *
     * @return a byte array ready for network transmission
     */
    public byte[] toBytes() {
        int payloadSize = 1 + data.length;
        ByteBuffer buffer = ByteBuffer.allocate(HEADER_SIZE + payloadSize);

        buffer.put(MAGIC);
        buffer.put(PROTOCOL_VERSION);
        buffer.putInt(payloadSize);
        buffer.put(type.getId());
        buffer.put(data);

        return buffer.array();
    }

    /**
     * Deserializes a frame from a byte array received from the wire.
     *
     * @param raw the raw bytes to deserialize (must not be null)
     * @return the parsed message frame
     * @throws IllegalArgumentException if the data is malformed (bad magic, unsupported version,
     *         invalid payload length, or truncated data)
     */
    public static MessageFrame fromBytes(byte[] raw) {
        Objects.requireNonNull(raw, "raw bytes must not be null");

        if (raw.length < HEADER_SIZE + MIN_PAYLOAD_SIZE) {
            throw new IllegalArgumentException(
                    "Frame too small: " + raw.length + " bytes (minimum: " + (HEADER_SIZE + MIN_PAYLOAD_SIZE) + ")");
        }

        ByteBuffer buffer = ByteBuffer.wrap(raw);

        byte[] magic = new byte[4];
        buffer.get(magic);
        if (!Arrays.equals(magic, MAGIC)) {
            throw new IllegalArgumentException(
                    "Invalid magic bytes: expected P2PF, got " + new String(magic, StandardCharsets.US_ASCII));
        }

        byte version = buffer.get();
        if (version != PROTOCOL_VERSION) {
            throw new IllegalArgumentException(
                    "Unsupported protocol version: " + version + " (expected: " + PROTOCOL_VERSION + ")");
        }

        int payloadLength = buffer.getInt();
        if (payloadLength < MIN_PAYLOAD_SIZE) {
            throw new IllegalArgumentException("Payload length too small: " + payloadLength);
        }
        if (payloadLength > MAX_PAYLOAD_SIZE) {
            throw new IllegalArgumentException(
                    "Payload length " + payloadLength + " exceeds maximum " + MAX_PAYLOAD_SIZE);
        }
        if (buffer.remaining() < payloadLength) {
            throw new IllegalArgumentException(
                    "Incomplete frame: expected " + payloadLength + " payload bytes, got " + buffer.remaining());
        }

        byte typeId = buffer.get();
        MessageType type = MessageType.fromId(typeId);

        byte[] data = new byte[payloadLength - 1];
        buffer.get(data);

        return new MessageFrame(type, data);
    }

    // --- Factory methods ---

    /**
     * Creates a heartbeat frame with empty payload.
     *
     * @return a new HEARTBEAT frame
     */
    public static MessageFrame heartbeat() {
        return new MessageFrame(MessageType.HEARTBEAT, new byte[0]);
    }

    /**
     * Creates a heartbeat acknowledgment frame with empty payload.
     *
     * @return a new HEARTBEAT_ACK frame
     */
    public static MessageFrame heartbeatAck() {
        return new MessageFrame(MessageType.HEARTBEAT_ACK, new byte[0]);
    }

    /**
     * Creates an error frame with the given error message as payload.
     *
     * @param message the error message (encoded as UTF-8)
     * @return a new ERROR frame
     */
    public static MessageFrame error(String message) {
        Objects.requireNonNull(message, "message must not be null");
        return new MessageFrame(MessageType.ERROR, message.getBytes(StandardCharsets.UTF_8));
    }

    // --- Object overrides ---

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof MessageFrame other)) return false;
        return type == other.type && Arrays.equals(data, other.data);
    }

    @Override
    public int hashCode() {
        return Objects.hash(type, Arrays.hashCode(data));
    }

    @Override
    public String toString() {
        return String.format("MessageFrame[type=%s, dataLen=%d, wireSize=%d]",
                type, data.length, getWireSize());
    }
}
