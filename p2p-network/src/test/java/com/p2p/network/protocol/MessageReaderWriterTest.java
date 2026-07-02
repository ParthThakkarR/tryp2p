package com.p2p.network.protocol;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.DisplayName;

import java.io.*;

import static org.junit.jupiter.api.Assertions.*;

@DisplayName("MessageReader/Writer Tests")
class MessageReaderWriterTest {

    @Test
    @DisplayName("write then read round-trips correctly")
    void writeReadRoundTrip() throws Exception {
        ByteArrayOutputStream baos = new ByteArrayOutputStream();
        MessageWriter writer = new MessageWriter(baos);

        // HEARTBEAT frames are filtered out by readFrame() — use only
        // application-level types for the round-trip test.
        MessageFrame frame1 = new MessageFrame(MessageType.ERROR, "test error".getBytes());
        MessageFrame frame2 = new MessageFrame(MessageType.CHUNK_DATA, new byte[1024]);
        MessageFrame frame3 = new MessageFrame(MessageType.TRANSFER_REQUEST, "payload".getBytes());

        writer.writeFrame(frame1);
        writer.writeFrame(frame2);
        writer.writeFrame(frame3);
        writer.close();

        ByteArrayInputStream bais = new ByteArrayInputStream(baos.toByteArray());
        MessageReader reader = new MessageReader(bais);

        MessageFrame r1 = reader.readFrame();
        MessageFrame r2 = reader.readFrame();
        MessageFrame r3 = reader.readFrame();

        assertEquals(frame1, r1);
        assertEquals(frame2, r2);
        assertEquals(frame3, r3);

        // Next read should return null (EOF)
        assertNull(reader.readFrame());
        reader.close();
    }

    @Test
    @DisplayName("heartbeat frames are silently filtered out")
    void heartbeatFiltered() throws Exception {
        ByteArrayOutputStream baos = new ByteArrayOutputStream();
        MessageWriter writer = new MessageWriter(baos);

        MessageFrame heartbeat = new MessageFrame(MessageType.HEARTBEAT, new byte[0]);
        MessageFrame data = new MessageFrame(MessageType.ERROR, "payload".getBytes());

        writer.writeFrame(heartbeat);
        writer.writeFrame(data);
        writer.close();

        ByteArrayInputStream bais = new ByteArrayInputStream(baos.toByteArray());
        MessageReader reader = new MessageReader(bais);

        // Heartbeat should be skipped, only ERROR frame should be read
        MessageFrame r1 = reader.readFrame();
        assertEquals(data, r1);

        assertNull(reader.readFrame());
        reader.close();
    }

    @Test
    @DisplayName("reader returns null on empty stream")
    void emptyStream() throws Exception {
        MessageReader reader = new MessageReader(new ByteArrayInputStream(new byte[0]));
        assertNull(reader.readFrame());
        reader.close();
    }

    @Test
    @DisplayName("reader throws on corrupt magic bytes")
    void corruptMagic() throws Exception {
        byte[] corrupt = {0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x40};
        MessageReader reader = new MessageReader(new ByteArrayInputStream(corrupt));
        assertThrows(IOException.class, reader::readFrame);
        reader.close();
    }
}
