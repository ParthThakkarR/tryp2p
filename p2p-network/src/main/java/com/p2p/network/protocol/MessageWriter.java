package com.p2p.network.protocol;

import java.io.*;
import java.net.Socket;
import java.util.Objects;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * Writes {@link MessageFrame} instances to an {@link OutputStream}.
 *
 * <p>Thread-safe — all write operations are synchronized to prevent concurrent
 * access to the underlying stream from multiple virtual threads.
 */
public final class MessageWriter implements Closeable, Flushable {

    private static final Logger log = LoggerFactory.getLogger(MessageWriter.class);

    private final DataOutputStream out;

    /**
     * Creates a MessageWriter wrapping the given output stream.
     * The stream is wrapped in a {@link BufferedOutputStream} and {@link DataOutputStream}.
     *
     * @param outputStream the target output stream (must not be null)
     */
    public MessageWriter(OutputStream outputStream) {
        this.out = new DataOutputStream(
                new BufferedOutputStream(Objects.requireNonNull(outputStream, "outputStream required")));
    }

    /**
     * Creates a MessageWriter from a connected socket's output stream.
     *
     * @param socket the connected socket
     * @throws IOException if the socket's output stream cannot be obtained
     */
    public MessageWriter(Socket socket) throws IOException {
        this(socket.getOutputStream());
    }

    // --- Write ---

    /**
     * Writes a complete message frame to the stream.
     *
     * <p>The frame is serialized via {@link MessageFrame#toBytes()} and flushed
     * immediately to ensure timely delivery.
     *
     * @param frame the frame to write (must not be null)
     * @throws IOException if an I/O error occurs
     */
    public synchronized void writeFrame(MessageFrame frame) throws IOException {
        Objects.requireNonNull(frame, "frame must not be null");

        byte[] wireBytes = frame.toBytes();
        out.write(wireBytes);
        out.flush();

        log.trace("Wrote frame: {}", frame);
    }

    // --- Flush / Close ---

    @Override
    public synchronized void flush() throws IOException {
        out.flush();
    }

    @Override
    public synchronized void close() throws IOException {
        out.close();
    }
}
