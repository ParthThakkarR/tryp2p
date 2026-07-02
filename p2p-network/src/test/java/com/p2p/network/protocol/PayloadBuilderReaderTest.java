package com.p2p.network.protocol;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.DisplayName;

import static org.junit.jupiter.api.Assertions.*;

@DisplayName("PayloadBuilder/Reader Tests")
class PayloadBuilderReaderTest {

    @Test
    @DisplayName("round-trip all field types")
    void roundTripAllTypes() {
        byte[] payload = new PayloadBuilder()
                .writeString("hello world")
                .writeInt(42)
                .writeLong(123456789L)
                .writeBoolean(true)
                .writeByte((byte) 0xFF)
                .writeBytes(new byte[]{1, 2, 3, 4, 5})
                .writeString("")
                .writeBoolean(false)
                .build();

        PayloadReader reader = new PayloadReader(payload);
        assertEquals("hello world", reader.readString());
        assertEquals(42, reader.readInt());
        assertEquals(123456789L, reader.readLong());
        assertTrue(reader.readBoolean());
        assertEquals((byte) 0xFF, reader.readByte());
        assertArrayEquals(new byte[]{1, 2, 3, 4, 5}, reader.readBytes());
        assertEquals("", reader.readString());
        assertFalse(reader.readBoolean());
        assertFalse(reader.hasRemaining());
    }

    @Test
    @DisplayName("handles unicode strings")
    void unicodeStrings() {
        byte[] payload = new PayloadBuilder()
                .writeString("日本語テスト")
                .writeString("émojis: 🎉🚀")
                .build();

        PayloadReader reader = new PayloadReader(payload);
        assertEquals("日本語テスト", reader.readString());
        assertEquals("émojis: 🎉🚀", reader.readString());
    }

    @Test
    @DisplayName("handles null values gracefully")
    void nullValues() {
        byte[] payload = new PayloadBuilder()
                .writeString(null)
                .writeBytes(null)
                .build();

        PayloadReader reader = new PayloadReader(payload);
        assertEquals("", reader.readString());
        assertArrayEquals(new byte[0], reader.readBytes());
    }

    @Test
    @DisplayName("auto-grows buffer for large payloads")
    void autoGrow() {
        PayloadBuilder builder = new PayloadBuilder(16); // Start small
        byte[] largeData = new byte[8192];
        builder.writeBytes(largeData);
        builder.writeString("after large data");

        byte[] payload = builder.build();
        PayloadReader reader = new PayloadReader(payload);
        assertArrayEquals(largeData, reader.readBytes());
        assertEquals("after large data", reader.readString());
    }
}
