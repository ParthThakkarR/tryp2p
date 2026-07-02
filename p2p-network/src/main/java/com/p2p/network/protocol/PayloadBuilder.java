package com.p2p.network.protocol;

import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.util.Objects;

/**
 * Fluent builder for constructing binary payloads for protocol messages.
 *
 * <p>Supports writing strings (UTF-8 with 2-byte length prefix), byte arrays
 * (with 4-byte length prefix), and primitive types. The buffer auto-expands
 * as needed. Use {@link #build()} to obtain the final byte array.
 */
public final class PayloadBuilder {

    private ByteBuffer buffer;

    /**
     * Creates a new builder with a default initial capacity of 4096 bytes.
     */
    public PayloadBuilder() {
        this(4096);
    }

    /**
     * Creates a new builder with the specified initial capacity.
     *
     * @param initialCapacity the initial buffer capacity in bytes
     */
    public PayloadBuilder(int initialCapacity) {
        this.buffer = ByteBuffer.allocate(initialCapacity);
    }

    // --- Write methods ---

    /**
     * Writes a UTF-8 string, prefixed with a 2-byte unsigned short length.
     *
     * @param value the string to write (null is treated as empty string)
     * @return this builder for method chaining
     */
    public PayloadBuilder writeString(String value) {
        byte[] bytes = (value != null ? value : "").getBytes(StandardCharsets.UTF_8);
        ensureCapacity(2 + bytes.length);
        buffer.putShort((short) bytes.length);
        buffer.put(bytes);
        return this;
    }

    /**
     * Writes a raw byte array, prefixed with a 4-byte int length.
     *
     * @param value the byte array to write (null is treated as empty array)
     * @return this builder for method chaining
     */
    public PayloadBuilder writeBytes(byte[] value) {
        byte[] data = value != null ? value : new byte[0];
        ensureCapacity(4 + data.length);
        buffer.putInt(data.length);
        buffer.put(data);
        return this;
    }

    /**
     * Writes a single byte.
     *
     * @param value the byte value to write
     * @return this builder for method chaining
     */
    public PayloadBuilder writeByte(byte value) {
        ensureCapacity(1);
        buffer.put(value);
        return this;
    }

    /**
     * Writes a 4-byte integer (big-endian).
     *
     * @param value the integer value to write
     * @return this builder for method chaining
     */
    public PayloadBuilder writeInt(int value) {
        ensureCapacity(4);
        buffer.putInt(value);
        return this;
    }

    /**
     * Writes an 8-byte long (big-endian).
     *
     * @param value the long value to write
     * @return this builder for method chaining
     */
    public PayloadBuilder writeLong(long value) {
        ensureCapacity(8);
        buffer.putLong(value);
        return this;
    }

    /**
     * Writes a boolean as a single byte (1 for {@code true}, 0 for {@code false}).
     *
     * @param value the boolean value to write
     * @return this builder for method chaining
     */
    public PayloadBuilder writeBoolean(boolean value) {
        ensureCapacity(1);
        buffer.put(value ? (byte) 1 : (byte) 0);
        return this;
    }

    // --- Build ---

    /**
     * Returns the built payload as a byte array.
     *
     * @return the accumulated bytes
     */
    public byte[] build() {
        byte[] result = new byte[buffer.position()];
        buffer.flip();
        buffer.get(result);
        return result;
    }

    // --- Internal ---

    /**
     * Ensures the buffer has at least {@code additional} bytes remaining,
     * doubling capacity if necessary.
     */
    private void ensureCapacity(int additional) {
        if (buffer.remaining() < additional) {
            int newCapacity = Math.max(buffer.capacity() * 2, buffer.position() + additional);
            ByteBuffer newBuffer = ByteBuffer.allocate(newCapacity);
            buffer.flip();
            newBuffer.put(buffer);
            buffer = newBuffer;
        }
    }
}
