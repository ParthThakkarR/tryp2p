package com.p2p.core.util;

import java.util.Locale;

/**
 * Utility for detecting the current platform and providing OS-specific information.
 * Thread-safe — all results are computed once and cached in the static initializer.
 */
public final class PlatformUtils {

    /** Supported operating system families. */
    public enum OSFamily {
        WINDOWS, MACOS, LINUX, UNKNOWN
    }

    // --- Cached Platform Info ---

    private static final OSFamily CURRENT_OS;
    private static final String OS_NAME;
    private static final String OS_VERSION;
    private static final String OS_ARCH;

    static {
        OS_NAME = System.getProperty("os.name", "unknown");
        OS_VERSION = System.getProperty("os.version", "unknown");
        OS_ARCH = System.getProperty("os.arch", "unknown");

        String osLower = OS_NAME.toLowerCase(Locale.ENGLISH);
        if (osLower.contains("win")) {
            CURRENT_OS = OSFamily.WINDOWS;
        } else if (osLower.contains("mac") || osLower.contains("darwin")) {
            CURRENT_OS = OSFamily.MACOS;
        } else if (osLower.contains("nux") || osLower.contains("nix") || osLower.contains("aix")) {
            CURRENT_OS = OSFamily.LINUX;
        } else {
            CURRENT_OS = OSFamily.UNKNOWN;
        }
    }

    private PlatformUtils() {}

    // --- OS Detection ---

    /** Returns the detected operating system family. */
    public static OSFamily getCurrentOS() { return CURRENT_OS; }

    /** Returns the full OS name string (e.g., "Windows 11"). */
    public static String getOSName() { return OS_NAME; }

    /** Returns the OS version string. */
    public static String getOSVersion() { return OS_VERSION; }

    /** Returns the OS architecture string (e.g., "amd64"). */
    public static String getOSArch() { return OS_ARCH; }

    /** Returns true if the current OS is Windows. */
    public static boolean isWindows() { return CURRENT_OS == OSFamily.WINDOWS; }

    /** Returns true if the current OS is macOS. */
    public static boolean isMacOS() { return CURRENT_OS == OSFamily.MACOS; }

    /** Returns true if the current OS is Linux. */
    public static boolean isLinux() { return CURRENT_OS == OSFamily.LINUX; }

    // --- Platform Info ---

    /**
     * Returns a human-readable platform string (e.g., "Windows 11 (amd64)").
     *
     * @return formatted platform string
     */
    public static String getPlatformString() {
        return OS_NAME + " " + OS_VERSION + " (" + OS_ARCH + ")";
    }

    // --- System Resources ---

    /**
     * Returns the number of available processor cores.
     *
     * @return available processors
     */
    public static int getAvailableProcessors() {
        return Runtime.getRuntime().availableProcessors();
    }

    /**
     * Returns the maximum heap memory in bytes.
     *
     * @return max heap memory
     */
    public static long getMaxMemory() {
        return Runtime.getRuntime().maxMemory();
    }

    /**
     * Returns the approximate free heap memory in bytes.
     *
     * @return free heap memory
     */
    public static long getFreeMemory() {
        return Runtime.getRuntime().freeMemory();
    }
}
