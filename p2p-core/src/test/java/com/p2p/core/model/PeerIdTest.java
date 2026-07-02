package com.p2p.core.model;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.DisplayName;

import static org.junit.jupiter.api.Assertions.*;

@DisplayName("PeerId Tests")
class PeerIdTest {

    @Test
    @DisplayName("generate() creates unique IDs")
    void generateCreatesUniqueIds() {
        PeerId id1 = PeerId.generate();
        PeerId id2 = PeerId.generate();
        assertNotEquals(id1, id2);
    }

    @Test
    @DisplayName("fromHex() round-trips correctly")
    void fromHexRoundTrips() {
        PeerId original = PeerId.generate();
        String hex = original.toHex();
        PeerId restored = PeerId.fromHex(hex);
        assertEquals(original, restored);
        assertEquals(hex, restored.toHex());
    }

    @Test
    @DisplayName("fromBytes() round-trips correctly")
    void fromBytesRoundTrips() {
        PeerId original = PeerId.generate();
        byte[] bytes = original.toBytes();
        PeerId restored = PeerId.fromBytes(bytes);
        assertEquals(original, restored);
    }

    @Test
    @DisplayName("toBytes() returns defensive copy")
    void toBytesReturnsDefensiveCopy() {
        PeerId id = PeerId.generate();
        byte[] bytes1 = id.toBytes();
        byte[] bytes2 = id.toBytes();
        assertNotSame(bytes1, bytes2);
        assertArrayEquals(bytes1, bytes2);

        // Modifying returned array shouldn't affect internal state
        bytes1[0] = (byte) 0xFF;
        assertArrayEquals(bytes2, id.toBytes());
    }

    @Test
    @DisplayName("toHex() returns 32-char lowercase hex string")
    void toHexFormat() {
        PeerId id = PeerId.generate();
        String hex = id.toHex();
        assertEquals(32, hex.length());
        assertTrue(hex.matches("[0-9a-f]{32}"));
    }

    @Test
    @DisplayName("toShortString() returns first 8 hex chars")
    void toShortString() {
        PeerId id = PeerId.generate();
        assertEquals(8, id.toShortString().length());
        assertTrue(id.toHex().startsWith(id.toShortString()));
    }

    @Test
    @DisplayName("fromHex() rejects invalid input")
    void fromHexRejectsInvalid() {
        assertThrows(IllegalArgumentException.class, () -> PeerId.fromHex("short"));
        assertThrows(IllegalArgumentException.class, () -> PeerId.fromHex("zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz"));
        assertThrows(NullPointerException.class, () -> PeerId.fromHex(null));
    }

    @Test
    @DisplayName("fromBytes() rejects invalid length")
    void fromBytesRejectsInvalidLength() {
        assertThrows(IllegalArgumentException.class, () -> PeerId.fromBytes(new byte[15]));
        assertThrows(IllegalArgumentException.class, () -> PeerId.fromBytes(new byte[17]));
        assertThrows(IllegalArgumentException.class, () -> PeerId.fromBytes(null));
    }

    @Test
    @DisplayName("equals() and hashCode() contract")
    void equalsAndHashCode() {
        PeerId id1 = PeerId.generate();
        PeerId id2 = PeerId.fromHex(id1.toHex());
        PeerId id3 = PeerId.generate();

        assertEquals(id1, id2);
        assertEquals(id1.hashCode(), id2.hashCode());
        assertNotEquals(id1, id3);
        assertNotEquals(id1, null);
        assertNotEquals(id1, "not a PeerId");
    }

    @Test
    @DisplayName("compareTo() provides consistent ordering")
    void compareToOrdering() {
        PeerId id1 = PeerId.generate();
        PeerId id2 = PeerId.generate();
        // compareTo should be consistent with equals
        if (id1.equals(id2)) {
            assertEquals(0, id1.compareTo(id2));
        } else {
            assertNotEquals(0, id1.compareTo(id2));
        }
        // Antisymmetric
        assertEquals(-id1.compareTo(id2), id2.compareTo(id1));
    }

    @Test
    @DisplayName("toString() includes hex representation")
    void toStringFormat() {
        PeerId id = PeerId.generate();
        String str = id.toString();
        assertTrue(str.contains(id.toHex()));
        assertTrue(str.startsWith("PeerId["));
    }
}
