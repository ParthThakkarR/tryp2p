package com.p2p.network.protocol;

import java.io.*;
import java.net.Socket;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Objects;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * Reads {@link MessageFrame} instances from an {@link InputStream} with proper framing.
 *
 * <p>Handles partial reads via {@link DataInputStream#readFully} and validates frame
 * integrity (magic bytes, protocol version, payload bounds).
 *
 * <p>Heartbeat frames ({@link MessageType#HEARTBEAT} and {@link MessageType#HEARTBEAT_ACK})
 * are silently filtered out in {@link #readFrame()} so the application never sees them.
 * This allows the heartbeat monitor to share the same socket reader/writer pair without
 * corrupting the application-level message stream.
 *
 * <p>Not thread-safe for concurrent access; intended for single-reader usage per socket.
 */
public final class MessageReader implements Closeable {

    private static final Logger log = LoggerFactory.getLogger(MessageReader.class);

    private final DataInputStream in;

    /**
     * Creates a MessageReader wrapping the given input stream.
     * The stream is wrapped in a {@link BufferedInputStream} and {@link DataInputStream}.
     *
     * @param inputStream the source input stream (must not be null)
     */
    public MessageReader(InputStream inputStream) {
        this.in = new DataInputStream(
                new BufferedInputStream(Objects.requireNonNull(inputStream, "inputStream required")));
    }

    /**
     * Creates a MessageReader from a connected socket's input stream.
     *
     * @param socket the connected socket
     * @throws IOException if the socket's input stream cannot be obtained
     */
    public MessageReader(Socket socket) throws IOException {
        this(socket.getInputStream());
    }

    // --- Frame reading ---

    /**
     * Reads the next non-heartbeat message frame from the stream.
     *
     * <p>Heartbeat frames are silently consumed to avoid corrupting the application-level
     * data stream. Only application frames are returned to the caller.
     *
     * @return the next application {@link MessageFrame}, or {@code null} if the stream
     *         has been closed gracefully (EOF)
     * @throws IOException if an I/O error occurs or the frame is malformed
     */
    public MessageFrame readFrame() throws IOException {
        while (true) {
            MessageFrame frame = readRawFrame();
            if (frame == null) return null;

            if (frame.getType() == MessageType.HEARTBEAT) {
                log.trace("Filtered out HEARTBEAT frame");
                continue;
            }
            if (frame.getType() == MessageType.HEARTBEAT_ACK) {
                log.trace("Filtered out HEARTBEAT_ACK frame");
                continue;
            }
            return frame;
        }
    }

    /**
     * Reads a raw frame from the stream without any heartbeat filtering.
     *
     * <p>Performs full frame validation: magic bytes check, protocol version check,
     * payload length bounds, and type resolution.
     *
     * @return the raw {@link MessageFrame}, or {@code null} on EOF
     * @throws IOException if the frame is malformed or an I/O error occurs
     */
    private MessageFrame readRawFrame() throws IOException {
        byte[] magic = new byte[4];
        try {
            in.readFully(magic);
        } catch (EOFException e) {
            return null;
        }

        if (!Arrays.equals(magic, MessageFrame.MAGIC)) {
            throw new IOException("Invalid magic bytes: expected P2PF, got "
                    + new String(magic, StandardCharsets.US_ASCII));
        }

        byte version = in.readByte();
        if (version != MessageFrame.PROTOCOL_VERSION) {
            throw new IOException("Unsupported protocol version: " + version);
        }

        int payloadLength = in.readInt();
        if (payloadLength < MessageFrame.MIN_PAYLOAD_SIZE) {
            throw new IOException("Payload length too small: " + payloadLength);
        }
        if (payloadLength > MessageFrame.MAX_PAYLOAD_SIZE) {
            throw new IOException("Payload length exceeds maximum: " + payloadLength);
        }

        byte typeId = in.readByte();
        MessageType type = MessageType.fromId(typeId);

        byte[] data = new byte[payloadLength - 1];
        if (data.length > 0) {
            in.readFully(data);
        }

        MessageFrame frame = new MessageFrame(type, data);
        log.trace("Read raw frame: {}", frame);
        return frame;
    }

    // --- Lifecycle ---

    @Override
    public void close() throws IOException {
        in.close();
    }
}
