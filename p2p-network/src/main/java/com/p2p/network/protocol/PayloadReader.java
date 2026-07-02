package com.p2p.network.protocol;

import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;

/**
 * Reads typed fields from a binary payload, in the format written by {@link PayloadBuilder}.
 *
 * <p>Supports reading strings (UTF-8 with 2-byte length prefix), byte arrays
 * (with 4-byte length prefix), and primitive types. This is the deserialization
 * counterpart of {@link PayloadBuilder}.
 */
public final class PayloadReader {

    private final ByteBuffer buffer;

    /**
     * Creates a reader over the given byte array.
     *
     * @param data the payload bytes to read from
     */
    public PayloadReader(byte[] data) {
        this.buffer = ByteBuffer.wrap(data);
    }

    // --- Read methods ---

    /**
     * Reads a UTF-8 string prefixed with a 2-byte unsigned short length.
     *
     * @return the decoded string
     */
    public String readString() {
        int length = Short.toUnsignedInt(buffer.getShort());
        byte[] bytes = new byte[length];
        buffer.get(bytes);
        return new String(bytes, StandardCharsets.UTF_8);
    }

    /**
     * Reads a raw byte array prefixed with a 4-byte int length.
     *
     * @return the byte array
     * @throws IllegalArgumentException if the length is negative or exceeds
     *         {@link MessageFrame#MAX_PAYLOAD_SIZE}
     */
    public byte[] readBytes() {
        int length = buffer.getInt();
        if (length < 0 || length > MessageFrame.MAX_PAYLOAD_SIZE) {
            throw new IllegalArgumentException("Invalid byte array length: " + length);
        }
        byte[] data = new byte[length];
        buffer.get(data);
        return data;
    }

    /**
     * Reads a single byte.
     *
     * @return the byte value
     */
    public byte readByte() {
        return buffer.get();
    }

    /**
     * Reads a 4-byte integer (big-endian).
     *
     * @return the integer value
     */
    public int readInt() {
        return buffer.getInt();
    }

    /**
     * Reads an 8-byte long (big-endian).
     *
     * @return the long value
     */
    public long readLong() {
        return buffer.getLong();
    }

    /**
     * Reads a boolean encoded as a single byte (nonzero = {@code true}).
     *
     * @return the boolean value
     */
    public boolean readBoolean() {
        return buffer.get() != 0;
    }

    // --- State ---

    /**
     * Returns the number of bytes remaining in the buffer.
     *
     * @return remaining byte count
     */
    public int remaining() {
        return buffer.remaining();
    }

    /**
     * Returns {@code true} if there are more bytes available to read.
     *
     * @return true if bytes remain
     */
    public boolean hasRemaining() {
        return buffer.hasRemaining();
    }
}
