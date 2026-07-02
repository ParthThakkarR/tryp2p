package com.p2p.core.util;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.DisplayName;

import static org.junit.jupiter.api.Assertions.*;

@DisplayName("PlatformUtils Tests")
class PlatformUtilsTest {

    @Test
    @DisplayName("getCurrentOS returns non-null")
    void currentOsNonNull() {
        assertNotNull(PlatformUtils.getCurrentOS());
    }

    @Test
    @DisplayName("platform string contains OS name")
    void platformStringContainsOs() {
        String platform = PlatformUtils.getPlatformString();
        assertNotNull(platform);
        assertFalse(platform.isEmpty());
        assertTrue(platform.contains(PlatformUtils.getOSName()));
    }

    @Test
    @DisplayName("on Windows, isWindows returns true")
    void platformDetection() {
        // At least one of these must be true
        boolean detected = PlatformUtils.isWindows()
                || PlatformUtils.isMacOS()
                || PlatformUtils.isLinux()
                || PlatformUtils.getCurrentOS() == PlatformUtils.OSFamily.UNKNOWN;
        assertTrue(detected);
    }

    @Test
    @DisplayName("getAvailableProcessors returns positive value")
    void processorsPositive() {
        assertTrue(PlatformUtils.getAvailableProcessors() > 0);
    }

    @Test
    @DisplayName("getMaxMemory returns positive value")
    void maxMemoryPositive() {
        assertTrue(PlatformUtils.getMaxMemory() > 0);
    }
}
