package com.p2p.core.model;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.DisplayName;

import java.net.InetAddress;
import java.time.Instant;

import static org.junit.jupiter.api.Assertions.*;

@DisplayName("PeerInfo Tests")
class PeerInfoTest {

    private PeerInfo createTestPeer() throws Exception {
        return PeerInfo.builder()
                .peerId(PeerId.generate())
                .displayName("TestPeer")
                .address(InetAddress.getByName("192.168.1.100"))
                .port(9877)
                .operatingSystem("Windows 11")
                .appVersion("1.0.0")
                .lastSeen(Instant.now())
                .status(PeerStatus.ONLINE)
                .build();
    }

    @Test
    @DisplayName("builder creates valid PeerInfo")
    void builderCreatesValid() throws Exception {
        PeerInfo peer = createTestPeer();
        assertEquals("TestPeer", peer.getDisplayName());
        assertEquals(9877, peer.getPort());
        assertEquals(PeerStatus.ONLINE, peer.getStatus());
        assertNotNull(peer.getPeerId());
    }

    @Test
    @DisplayName("builder rejects null required fields")
    void builderRejectsNull() {
        assertThrows(NullPointerException.class, () -> PeerInfo.builder()
                .displayName("Test")
                .build());
    }

    @Test
    @DisplayName("builder rejects invalid port")
    void builderRejectsInvalidPort() {
        assertThrows(IllegalArgumentException.class, () -> PeerInfo.builder()
                .peerId(PeerId.generate())
                .displayName("Test")
                .address(InetAddress.getLoopbackAddress())
                .port(0)
                .operatingSystem("Linux")
                .appVersion("1.0.0")
                .build());

        assertThrows(IllegalArgumentException.class, () -> PeerInfo.builder()
                .peerId(PeerId.generate())
                .displayName("Test")
                .address(InetAddress.getLoopbackAddress())
                .port(70000)
                .operatingSystem("Linux")
                .appVersion("1.0.0")
                .build());
    }

    @Test
    @DisplayName("withRefreshed() updates lastSeen and status")
    void withRefreshedUpdates() throws Exception {
        PeerInfo original = createTestPeer().withStatus(PeerStatus.UNREACHABLE);
        assertEquals(PeerStatus.UNREACHABLE, original.getStatus());

        PeerInfo refreshed = original.withRefreshed();
        assertEquals(PeerStatus.ONLINE, refreshed.getStatus());
        assertTrue(refreshed.getLastSeen().isAfter(original.getLastSeen())
                || refreshed.getLastSeen().equals(original.getLastSeen()));
        // Identity preserved
        assertEquals(original.getPeerId(), refreshed.getPeerId());
    }

    @Test
    @DisplayName("equals() based on PeerId only")
    void equalsByPeerId() throws Exception {
        PeerId sharedId = PeerId.generate();
        PeerInfo peer1 = PeerInfo.builder()
                .peerId(sharedId)
                .displayName("Peer A")
                .address(InetAddress.getByName("192.168.1.1"))
                .port(9877)
                .operatingSystem("Linux")
                .appVersion("1.0.0")
                .build();

        PeerInfo peer2 = PeerInfo.builder()
                .peerId(sharedId)
                .displayName("Peer B")
                .address(InetAddress.getByName("10.0.0.1"))
                .port(9878)
                .operatingSystem("macOS")
                .appVersion("2.0.0")
                .build();

        assertEquals(peer1, peer2);
        assertEquals(peer1.hashCode(), peer2.hashCode());
    }

    @Test
    @DisplayName("toString() contains key info")
    void toStringContainsInfo() throws Exception {
        PeerInfo peer = createTestPeer();
        String str = peer.toString();
        assertTrue(str.contains("TestPeer"));
        assertTrue(str.contains("9877"));
        assertTrue(str.contains("ONLINE"));
    }
}
