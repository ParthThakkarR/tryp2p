package com.p2p.network.protocol;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.DisplayName;

import static org.junit.jupiter.api.Assertions.*;

@DisplayName("MessageFrame Tests")
class MessageFrameTest {

    @Test
    @DisplayName("round-trip serialization works for all message types")
    void roundTripAllTypes() {
        for (MessageType type : MessageType.values()) {
            byte[] testData = ("test-data-for-" + type.name()).getBytes();
            MessageFrame original = new MessageFrame(type, testData);
            byte[] wire = original.toBytes();
            MessageFrame restored = MessageFrame.fromBytes(wire);

            assertEquals(original.getType(), restored.getType());
            assertArrayEquals(original.getData(), restored.getData());
        }
    }

    @Test
    @DisplayName("empty payload round-trips correctly")
    void emptyPayload() {
        MessageFrame frame = new MessageFrame(MessageType.HEARTBEAT, new byte[0]);
        byte[] wire = frame.toBytes();
        MessageFrame restored = MessageFrame.fromBytes(wire);

        assertEquals(MessageType.HEARTBEAT, restored.getType());
        assertEquals(0, restored.getDataLength());
    }

    @Test
    @DisplayName("getWireSize is consistent with toBytes length")
    void wireSizeConsistent() {
        byte[] data = new byte[256];
        MessageFrame frame = new MessageFrame(MessageType.CHUNK_DATA, data);
        assertEquals(frame.getWireSize(), frame.toBytes().length);
    }

    @Test
    @DisplayName("rejects frame with invalid magic bytes")
    void rejectsInvalidMagic() {
        byte[] bad = new byte[20];
        bad[0] = 'X'; bad[1] = 'Y'; bad[2] = 'Z'; bad[3] = 'W';
        assertThrows(IllegalArgumentException.class, () -> MessageFrame.fromBytes(bad));
    }

    @Test
    @DisplayName("rejects frame with unsupported version")
    void rejectsUnsupportedVersion() {
        MessageFrame frame = new MessageFrame(MessageType.HEARTBEAT, new byte[0]);
        byte[] wire = frame.toBytes();
        wire[4] = (byte) 0x99; // corrupt version byte
        assertThrows(IllegalArgumentException.class, () -> MessageFrame.fromBytes(wire));
    }

    @Test
    @DisplayName("rejects frame that is too small")
    void rejectsTooSmall() {
        assertThrows(IllegalArgumentException.class, () -> MessageFrame.fromBytes(new byte[5]));
    }

    @Test
    @DisplayName("rejects null input")
    void rejectsNull() {
        assertThrows(NullPointerException.class, () -> MessageFrame.fromBytes(null));
    }

    @Test
    @DisplayName("rejects payload exceeding maximum size")
    void rejectsOversizedPayload() {
        assertThrows(IllegalArgumentException.class, () ->
                new MessageFrame(MessageType.CHUNK_DATA, new byte[MessageFrame.MAX_PAYLOAD_SIZE]));
    }

    @Test
    @DisplayName("factory methods create correct types")
    void factoryMethods() {
        assertEquals(MessageType.HEARTBEAT, MessageFrame.heartbeat().getType());
        assertEquals(MessageType.HEARTBEAT_ACK, MessageFrame.heartbeatAck().getType());

        MessageFrame err = MessageFrame.error("test error");
        assertEquals(MessageType.ERROR, err.getType());
        assertEquals("test error", err.getDataAsString());
    }

    @Test
    @DisplayName("getData returns defensive copy")
    void defensiveCopy() {
        byte[] original = {1, 2, 3};
        MessageFrame frame = new MessageFrame(MessageType.HEARTBEAT, original);
        byte[] copy = frame.getData();
        copy[0] = 99;
        assertEquals(1, frame.getData()[0]); // Internal state unchanged
    }
}
