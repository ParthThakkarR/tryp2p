package com.p2p.security.firewall;

import com.p2p.core.util.PlatformUtils;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.net.InetAddress;
import java.time.Instant;
import java.util.ArrayList;
import java.util.List;
import java.util.Objects;

/**
 * Generates platform-specific firewall rules (netsh, iptables, pfctl)
 * for allowing or blocking P2P traffic to a given peer address and port.
 * Rules are returned as human-readable command strings suitable for
 * manual execution or display.
 */
public final class FirewallRuleGenerator {

    // --- Constants ---

    private static final Logger log = LoggerFactory.getLogger(FirewallRuleGenerator.class);
    private static final String COMMENT_PREFIX = "# P2P Firewall Rules";

    // --- Fields ---

    private final PlatformUtils.OSFamily osFamily;

    // --- Constructor ---

    /**
     * Constructs a generator that detects the current operating system
     * at creation time.
     */
    public FirewallRuleGenerator() {
        this.osFamily = PlatformUtils.getCurrentOS();
    }

    // --- Public API ---

    /**
     * Generates firewall rules to allow TCP traffic to/from a peer address
     * on the given port.
     *
     * @param peerAddress the peer's IP address
     * @param port        the local TCP port
     * @return a list of platform-specific firewall command strings
     */
    public List<String> generateAllowRules(InetAddress peerAddress, int port) {
        Objects.requireNonNull(peerAddress, "peerAddress must not be null");
        return switch (osFamily) {
            case WINDOWS -> generateWindowsRules(peerAddress, port);
            case LINUX -> generateLinuxRules(peerAddress, port);
            case MACOS -> generateMacRules(peerAddress, port);
            case UNKNOWN -> generateLinuxRules(peerAddress, port);
        };
    }

    /**
     * Generates firewall rules to block all TCP traffic to/from a peer address.
     *
     * @param peerAddress the peer's IP address
     * @param port        the port to reference in rule names (Windows)
     * @return a list of platform-specific firewall command strings
     */
    public List<String> generateBlockRules(InetAddress peerAddress, int port) {
        Objects.requireNonNull(peerAddress, "peerAddress must not be null");
        return switch (osFamily) {
            case WINDOWS -> generateWindowsBlockRules(peerAddress, port);
            case LINUX -> generateLinuxBlockRules(peerAddress, port);
            case MACOS -> generateMacBlockRules(peerAddress, port);
            case UNKNOWN -> generateLinuxBlockRules(peerAddress, port);
        };
    }

    /**
     * Formats a list of rule command strings for human-readable display,
     * prefixed with a header and generation timestamp.
     *
     * @param rules the list of rule command strings
     * @return a formatted display string
     */
    public String formatRulesForDisplay(List<String> rules) {
        Objects.requireNonNull(rules, "rules must not be null");
        StringBuilder sb = new StringBuilder();
        sb.append(COMMENT_PREFIX).append("\n");
        sb.append("# Generated: ").append(Instant.now()).append("\n\n");
        for (String rule : rules) {
            sb.append(rule).append("\n");
        }
        return sb.toString();
    }

    @Override
    public String toString() {
        return String.format("FirewallRuleGenerator[os=%s]", osFamily);
    }

    // --- Internal ---

    private List<String> generateWindowsRules(InetAddress peer, int port) {
        List<String> rules = new ArrayList<>();
        String ip = peer.getHostAddress();
        String ruleName = "P2P-Allow-" + ip.replace('.', '-');
        rules.add("netsh advfirewall firewall add rule name=\"" + ruleName
                + "\" dir=in action=allow protocol=tcp remoteip=" + ip + " localport=" + port);
        rules.add("netsh advfirewall firewall add rule name=\"" + ruleName
                + "-out\" dir=out action=allow protocol=tcp remoteip=" + ip + " remoteport=" + port);
        return rules;
    }

    private List<String> generateWindowsBlockRules(InetAddress peer, int port) {
        List<String> rules = new ArrayList<>();
        String ip = peer.getHostAddress();
        String ruleName = "P2P-Block-" + ip.replace('.', '-');
        rules.add("netsh advfirewall firewall add rule name=\"" + ruleName
                + "\" dir=in action=block protocol=tcp remoteip=" + ip);
        rules.add("netsh advfirewall firewall add rule name=\"" + ruleName
                + "-out\" dir=out action=block protocol=tcp remoteip=" + ip);
        return rules;
    }

    private List<String> generateLinuxRules(InetAddress peer, int port) {
        List<String> rules = new ArrayList<>();
        String ip = peer.getHostAddress();
        rules.add("iptables -A INPUT -s " + ip + " -p tcp --dport " + port + " -j ACCEPT");
        rules.add("iptables -A OUTPUT -d " + ip + " -p tcp --sport " + port + " -j ACCEPT");
        return rules;
    }

    private List<String> generateLinuxBlockRules(InetAddress peer, int port) {
        List<String> rules = new ArrayList<>();
        String ip = peer.getHostAddress();
        rules.add("iptables -A INPUT -s " + ip + " -j DROP");
        rules.add("iptables -A OUTPUT -d " + ip + " -j DROP");
        return rules;
    }

    private List<String> generateMacRules(InetAddress peer, int port) {
        List<String> rules = new ArrayList<>();
        String ip = peer.getHostAddress();
        rules.add("echo 'pass in from " + ip + " to any port " + port + " proto tcp' | pfctl -ef -");
        rules.add("echo 'pass out from any port " + port + " to " + ip + " proto tcp' | pfctl -ef -");
        return rules;
    }

    private List<String> generateMacBlockRules(InetAddress peer, int port) {
        List<String> rules = new ArrayList<>();
        String ip = peer.getHostAddress();
        rules.add("echo 'block in from " + ip + " to any' | pfctl -ef -");
        rules.add("echo 'block out from any to " + ip + "' | pfctl -ef -");
        return rules;
    }
}
