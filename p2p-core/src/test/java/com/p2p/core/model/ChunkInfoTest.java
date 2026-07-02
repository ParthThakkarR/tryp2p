package com.p2p.core.model;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.DisplayName;

import static org.junit.jupiter.api.Assertions.*;

@DisplayName("ChunkInfo Tests")
class ChunkInfoTest {

    @Test
    @DisplayName("uncompressed chunk has ratio 1.0")
    void uncompressedRatio() {
        ChunkInfo chunk = ChunkInfo.builder()
                .chunkIndex(0)
                .offset(0)
                .length(1024)
                .sha256Hash("abc123")
                .compressed(false)
                .build();

        assertEquals(1.0, chunk.getCompressionRatio());
        assertEquals(1024, chunk.getEffectiveLength());
    }

    @Test
    @DisplayName("compressed chunk calculates ratio correctly")
    void compressedRatio() {
        ChunkInfo chunk = ChunkInfo.builder()
                .chunkIndex(0)
                .offset(0)
                .length(1000)
                .sha256Hash("abc123")
                .compressed(true)
                .compressedLength(500)
                .build();

        assertEquals(0.5, chunk.getCompressionRatio(), 0.001);
        assertEquals(500, chunk.getEffectiveLength());
    }

    @Test
    @DisplayName("rejects negative chunk index")
    void rejectsNegativeIndex() {
        assertThrows(IllegalArgumentException.class, () -> ChunkInfo.builder()
                .chunkIndex(-1).offset(0).length(100).build());
    }

    @Test
    @DisplayName("rejects zero length")
    void rejectsZeroLength() {
        assertThrows(IllegalArgumentException.class, () -> ChunkInfo.builder()
                .chunkIndex(0).offset(0).length(0).build());
    }

    @Test
    @DisplayName("rejects compressed with zero compressed length")
    void rejectsCompressedWithZeroLength() {
        assertThrows(IllegalArgumentException.class, () -> ChunkInfo.builder()
                .chunkIndex(0).offset(0).length(1000)
                .compressed(true).compressedLength(0).build());
    }

    @Test
    @DisplayName("equals based on index, offset, and length")
    void equalsContract() {
        ChunkInfo c1 = ChunkInfo.builder().chunkIndex(0).offset(0).length(1024).build();
        ChunkInfo c2 = ChunkInfo.builder().chunkIndex(0).offset(0).length(1024)
                .sha256Hash("different").build();
        ChunkInfo c3 = ChunkInfo.builder().chunkIndex(1).offset(1024).length(1024).build();

        assertEquals(c1, c2);
        assertEquals(c1.hashCode(), c2.hashCode());
        assertNotEquals(c1, c3);
    }
}
