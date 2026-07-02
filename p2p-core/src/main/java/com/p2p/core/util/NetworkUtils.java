package com.p2p.core.util;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.net.*;
import java.util.ArrayList;
import java.util.Collections;
import java.util.Enumeration;
import java.util.List;

/**
 * Network utilities for cross-platform network interface enumeration
 * and address resolution. All methods are stateless and thread-safe.
 */
public final class NetworkUtils {

    private static final Logger log = LoggerFactory.getLogger(NetworkUtils.class);

    private NetworkUtils() {}

    // --- Interface Enumeration ---

    /**
     * Returns all active, non-loopback network interfaces that support multicast.
     *
     * @return unmodifiable list of eligible network interfaces
     */
    public static List<NetworkInterface> getActiveInterfaces() {
        List<NetworkInterface> result = new ArrayList<>();
        try {
            Enumeration<NetworkInterface> interfaces = NetworkInterface.getNetworkInterfaces();
            if (interfaces == null) return result;

            while (interfaces.hasMoreElements()) {
                NetworkInterface ni = interfaces.nextElement();
                try {
                    if (ni.isUp() && !ni.isLoopback() && ni.supportsMulticast()) {
                        result.add(ni);
                    }
                } catch (SocketException e) {
                    log.trace("Skipping interface {}: {}", ni.getDisplayName(), e.getMessage());
                }
            }
        } catch (SocketException e) {
            log.warn("Failed to enumerate network interfaces: {}", e.getMessage());
        }
        return Collections.unmodifiableList(result);
    }

    // --- Address Resolution ---

    /**
     * Returns the best (most likely LAN-reachable) local IPv4 address.
     * Prefers site-local addresses (192.168.x.x, 10.x.x.x, 172.16-31.x.x).
     *
     * @return the preferred local address
     */
    public static InetAddress getPreferredLocalAddress() {
        InetAddress fallback = InetAddress.getLoopbackAddress();

        for (NetworkInterface ni : getActiveInterfaces()) {
            Enumeration<InetAddress> addresses = ni.getInetAddresses();
            while (addresses.hasMoreElements()) {
                InetAddress addr = addresses.nextElement();
                if (addr instanceof Inet4Address && addr.isSiteLocalAddress()) {
                    return addr;
                }
            }
        }

        // Fallback: try connecting to a public address (doesn't send data)
        try (DatagramSocket socket = new DatagramSocket()) {
            socket.connect(InetAddress.getByName("8.8.8.8"), 53);
            InetAddress localAddr = socket.getLocalAddress();
            if (!localAddr.isAnyLocalAddress()) {
                return localAddr;
            }
        } catch (Exception e) {
            log.debug("Fallback address detection failed: {}", e.getMessage());
        }

        return fallback;
    }

    /**
     * Returns all IPv4 site-local addresses across all active interfaces.
     *
     * @return unmodifiable list of local addresses
     */
    public static List<InetAddress> getAllLocalAddresses() {
        List<InetAddress> result = new ArrayList<>();
        for (NetworkInterface ni : getActiveInterfaces()) {
            Enumeration<InetAddress> addresses = ni.getInetAddresses();
            while (addresses.hasMoreElements()) {
                InetAddress addr = addresses.nextElement();
                if (addr instanceof Inet4Address && !addr.isLoopbackAddress()) {
                    result.add(addr);
                }
            }
        }
        return Collections.unmodifiableList(result);
    }

    // --- Port Utilities ---

    /**
     * Checks if a given port is available for binding.
     *
     * @param port the port number to check
     * @return true if the port is available
     */
    public static boolean isPortAvailable(int port) {
        try (ServerSocket ss = new ServerSocket(port)) {
            ss.setReuseAddress(true);
            return true;
        } catch (Exception e) {
            return false;
        }
    }

    /**
     * Finds an available port starting from the given port.
     *
     * @param startPort   the port to start searching from
     * @param maxAttempts maximum ports to try
     * @return an available port, or -1 if none found
     */
    public static int findAvailablePort(int startPort, int maxAttempts) {
        for (int i = 0; i < maxAttempts; i++) {
            int port = startPort + i;
            if (port > 65535) break;
            if (isPortAvailable(port)) {
                return port;
            }
        }
        return -1;
    }

    // --- Formatting ---

    /**
     * Formats an InetAddress and port as a string (e.g., "192.168.1.5:9877").
     *
     * @param address the IP address
     * @param port    the port number
     * @return formatted endpoint string
     */
    public static String formatEndpoint(InetAddress address, int port) {
        return address.getHostAddress() + ":" + port;
    }
}
