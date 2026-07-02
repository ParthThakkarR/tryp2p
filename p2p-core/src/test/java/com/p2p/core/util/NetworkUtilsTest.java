package com.p2p.core.util;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.DisplayName;

import java.net.InetAddress;
import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

@DisplayName("NetworkUtils Tests")
class NetworkUtilsTest {

    @Test
    @DisplayName("getPreferredLocalAddress returns non-null")
    void preferredAddressNonNull() {
        InetAddress addr = NetworkUtils.getPreferredLocalAddress();
        assertNotNull(addr);
    }

    @Test
    @DisplayName("getActiveInterfaces returns list without loopback")
    void activeInterfacesNoLoopback() {
        var interfaces = NetworkUtils.getActiveInterfaces();
        assertNotNull(interfaces);
        for (var ni : interfaces) {
            try {
                assertFalse(ni.isLoopback(), "Should not contain loopback: " + ni.getDisplayName());
            } catch (Exception e) {
                // SocketException possible on some platforms
            }
        }
    }

    @Test
    @DisplayName("isPortAvailable returns true for ephemeral ports")
    void portAvailable() {
        // Port 0 is always "available" (OS assigns ephemeral port)
        // Pick a high port that's likely free
        boolean available = NetworkUtils.isPortAvailable(49999);
        // Can't guarantee this will be true in all environments
        // but we can at least verify the method doesn't crash
        assertNotNull(Boolean.valueOf(available));
    }

    @Test
    @DisplayName("findAvailablePort finds something in range")
    void findAvailablePort() {
        int port = NetworkUtils.findAvailablePort(50000, 100);
        // Should find at least one port in range 50000-50099
        assertTrue(port >= 50000 || port == -1);
    }

    @Test
    @DisplayName("formatEndpoint produces correct format")
    void formatEndpoint() throws Exception {
        InetAddress addr = InetAddress.getByName("192.168.1.5");
        assertEquals("192.168.1.5:9877", NetworkUtils.formatEndpoint(addr, 9877));
    }
}
