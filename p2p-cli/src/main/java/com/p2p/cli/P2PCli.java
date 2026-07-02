package com.p2p.cli;

import com.p2p.core.config.AppConfig;
import com.p2p.core.exception.P2PException;
import com.p2p.core.model.*;
import com.p2p.core.service.PeerDiscoveryService;
import com.p2p.core.util.FileUtils;
import com.p2p.core.util.NetworkUtils;
import com.p2p.core.util.PlatformUtils;
import com.p2p.network.discovery.MulticastDiscoveryService;
import com.p2p.network.protocol.MessageFrame;
import com.p2p.network.protocol.MessageReader;
import com.p2p.network.protocol.MessageType;
import com.p2p.network.protocol.MessageWriter;
import com.p2p.network.protocol.Messages;
import com.p2p.network.tcp.ConnectionManager;
import com.p2p.network.tcp.TcpClient;
import com.p2p.network.tcp.TcpServer;
import com.p2p.observability.audit.TransferAuditLogger;
import com.p2p.observability.health.HealthCheckServer;
import com.p2p.observability.metrics.MetricsExporter;
import com.p2p.observability.errors.ErrorReporter;
import com.p2p.observability.errors.FileErrorReporter;
import com.p2p.security.authlog.PeerAuthenticationLog;
import com.p2p.security.credential.CredentialStore;
import com.p2p.security.credential.CredentialStoreFactory;
import com.p2p.security.keyrotation.KeyRotationScheduler;
import com.p2p.security.pinning.CertificatePinningManager;
import com.p2p.security.ratelimit.RateLimiter;
import com.p2p.security.firewall.FirewallRuleGenerator;
import com.p2p.core.exception.ErrorCode;
import com.p2p.core.exception.ProtocolException;
import com.p2p.core.exception.TransferException;
import com.p2p.core.service.EncryptionService;
import com.p2p.crypto.AesGcmEncryptionService;
import com.p2p.crypto.ECDHKeyExchange;
import com.p2p.crypto.KeyDerivation;
import com.p2p.network.protocol.PayloadReader;
import com.p2p.transfer.compression.DeflateCompressionService;
import com.p2p.transfer.engine.ChunkedFileReader;
import com.p2p.transfer.engine.ChunkedFileWriter;
import com.p2p.transfer.engine.DirectoryScanner;
import com.p2p.transfer.engine.ParallelTransferEngine;
import com.p2p.transfer.resume.TransferResumeManager;
import com.p2p.core.service.CompressionService;

import picocli.CommandLine;
import picocli.CommandLine.Command;
import picocli.CommandLine.Option;
import picocli.CommandLine.Parameters;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import javax.crypto.KeyAgreement;
import java.io.IOException;
import java.net.InetAddress;
import java.net.Socket;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.security.*;
import java.security.spec.X509EncodedKeySpec;
import java.time.Instant;
import java.util.*;
import java.util.concurrent.Callable;
import java.util.concurrent.CopyOnWriteArrayList;

/**
 * Main CLI entry point for the P2P file transfer system.
 * <p>
 * This class serves as the picocli root command and wires together all subsystem
 * services (discovery, TCP server/client, encryption, transfer, security,
 * observability). It supports both one-shot command execution and an interactive
 * shell mode. All public static fields are initialized by {@link #initialize()}
 * and accessed by subcommands and the GUI. Thread-safe via static initialization;
 * mutable collections use CopyOnWriteArrayList for concurrent access.
 */
@Command(name = "p2p", mixinStandardHelpOptions = true,
         versionProvider = P2PCli.VersionProvider.class,
         description = "P2P File Transfer System - Secure peer-to-peer file sharing",
         subcommands = {
             P2PCli.DiscoverCommand.class,
             P2PCli.SendCommand.class,
             P2PCli.ReceiveCommand.class,
             P2PCli.StatusCommand.class,
             P2PCli.ConfigCommand.class,
             P2PCli.VersionCommand.class
         })
public class P2PCli implements Callable<Integer> {

    // --- Fields ---

    /** Application configuration loaded from CLI/env/file defaults. */
    public static AppConfig config = AppConfig.defaults();

    /** Locally-generated 16-byte peer identifier. */
    public static PeerId localPeerId = PeerId.generate();

    /** Audit log for peer authentication events. */
    static PeerAuthenticationLog authLog;

    /** Token-bucket rate limiter keyed by peer ID. */
    static RateLimiter rateLimiter;

    /** TOFU-based certificate pinning manager. */
    static CertificatePinningManager certPinning;

    /** Persistent credential store for peer credentials. */
    static CredentialStore credentialStore;

    /** Scheduled key rotation manager (7-day cycle with 1-hour fallback). */
    static KeyRotationScheduler keyRotation;

    /** Firewall rule generator for peer IPs. */
    static FirewallRuleGenerator firewallGenerator;

    /** Manager for resuming interrupted file transfers. */
    static TransferResumeManager resumeManager;

    /** Structured audit logger for transfer events. */
    static TransferAuditLogger auditLogger;

    /** HTTP health-check server exposing /health and /metrics endpoints. */
    static HealthCheckServer healthServer;

    /** Prometheus-format metrics exporter. */
    static MetricsExporter metricsExporter;

    /** Error reporter for persisting error contexts to disk. */
    static ErrorReporter errorReporter;

    /** Parallel transfer engine for concurrent chunked file transfers. */
    static ParallelTransferEngine transferEngine;

    /** Multicast-based LAN peer discovery service (239.255.80.50:9876). */
    public static MulticastDiscoveryService discovery;

    /** Manages TCP connections with heartbeats and reconnection. */
    public static ConnectionManager connectionManager;

    /** TCP server listening for incoming connections. */
    static TcpServer tcpServer;

    /** TCP client for outbound connections to peers. */
    static TcpClient tcpClient;

    /** Active transfer sessions (thread-safe, for GUI/status queries). */
    public static List<TransferSession> activeTransfers = new CopyOnWriteArrayList<>();

    /** Save directory for incoming transfers (set by --save-dir, defaults to ~/Desktop/downloads). */
    public static volatile Path receiveSaveDir = Paths.get(System.getProperty("user.home"), "Desktop", "downloads");

    /** Whether the receiver is actively listening for incoming transfers. */
    static volatile boolean receiving = false;

    /** Whether the CLI is running in interactive shell mode. */
    static volatile boolean interactiveMode = false;

    private static final Logger log = LoggerFactory.getLogger(P2PCli.class);

    @Option(names = {"--config"}, description = "Configuration file path")
    String configPath;

    // --- Initialization ---

    /**
     * Initializes all P2P subsystem services.
     * <p>
     * Creates the data directory, then instantiates and starts in order:
     * security services (auth log, rate limiter, key rotation, cert pinning,
     * credential store, firewall), observability (metrics, health, audit,
     * error reporting), networking (TCP client/server, connection manager
     * with heartbeats), transfer engine, and LAN discovery. Registers a
     * JVM shutdown hook to cleanly stop all services.
     *
     * @throws RuntimeException wrapping any checked exception during init
     */
    public static void initialize() {
        try {
            Path dataDir = config.getDataDirectory();
            Files.createDirectories(dataDir);

            authLog = new PeerAuthenticationLog(dataDir);
            rateLimiter = new RateLimiter(config.getDiscoveryRateLimit(),
                    config.getRateLimitBlockDuration(), dataDir);
            keyRotation = new KeyRotationScheduler(config);
            keyRotation.start();
            certPinning = new CertificatePinningManager(dataDir.resolve("config"),
                    keyRotation.getKeyRotationManager());
            credentialStore = CredentialStoreFactory.create(dataDir);
            firewallGenerator = new FirewallRuleGenerator();
            tryRegisterFirewall();
            resumeManager = new TransferResumeManager(dataDir, config.getResumeExpiryDays());
            auditLogger = new TransferAuditLogger(dataDir, config.getAuditRetentionDays());
            errorReporter = new FileErrorReporter(dataDir);

            if (config.isMetricsEnabled()) {
                metricsExporter = new MetricsExporter(config.getMetricsPort());
                metricsExporter.start();
            }

            healthServer = new HealthCheckServer(config.getMetricsPort() + 1, localPeerId, dataDir);
            healthServer.start();

            tcpClient = new TcpClient();
            connectionManager = new ConnectionManager(
                    config.getHeartbeatInterval().toMillis(),
                    config.getMissedHeartbeatsThreshold());

            tcpServer = new TcpServer(config, (socket, reader, writer) -> {
                try {
                    handleIncomingConnection(socket, reader, writer);
                } catch (Exception e) {
                    errorReporter.reportError(new ErrorReporter.ErrorContext(
                            Instant.now(), null, null, "CONNECTION_ERROR",
                            e.getMessage(), null, null, null));
                }
            });
            tcpServer.start();
            connectionManager.start();

            transferEngine = new ParallelTransferEngine(config);

            discovery = new MulticastDiscoveryService(config, localPeerId);
            discovery.start();

            Runtime.getRuntime().addShutdownHook(new Thread(P2PCli::shutdown));

        } catch (Exception e) {
            System.err.println("Failed to initialize: " + e.getMessage());
            e.printStackTrace();
            System.exit(1);
        }
    }

    /**
     * Gracefully shuts down all P2P subsystem services.
     * <p>
     * Stops discovery, connection manager, TCP server, key rotation, metrics,
     * health server, and transfer engine. Safe to call multiple times; guards
     * against null references.
     */
    public static void shutdown() {
        try {
            if (discovery != null) discovery.stop();
            if (connectionManager != null) connectionManager.stop();
            if (tcpServer != null) tcpServer.stop();
            if (keyRotation != null) keyRotation.close();
            if (metricsExporter != null) metricsExporter.close();
            if (healthServer != null) healthServer.close();
            if (transferEngine != null) transferEngine.close();
        } catch (Exception e) {
            System.err.println("Error during shutdown: " + e.getMessage());
        }
    }

    // --- Firewall ---

    /**
     * Attempts to register a Windows Firewall allow rule for our TCP port.
     * Silently ignores failures (e.g., if not running as admin).
     */
    static void tryRegisterFirewall() {
        if (PlatformUtils.getCurrentOS() != PlatformUtils.OSFamily.WINDOWS) return;
        try {
            String ruleName = "P2P Transfer Local";
            int port = config.getTcpPort();
            new ProcessBuilder(
                "netsh", "advfirewall", "firewall", "add", "rule",
                "name=" + ruleName, "dir=in", "action=allow", "protocol=tcp",
                "localport=" + port, "profile=private,domain"
            ).redirectErrorStream(true).redirectOutput(ProcessBuilder.Redirect.DISCARD).start();
            new ProcessBuilder(
                "netsh", "advfirewall", "firewall", "add", "rule",
                "name=" + ruleName + "-app", "dir=in", "action=allow", "protocol=tcp",
                "program=" + System.getProperty("java.home") + "\\bin\\java.exe",
                "profile=private,domain"
            ).redirectErrorStream(true).redirectOutput(ProcessBuilder.Redirect.DISCARD).start();
        } catch (Exception e) {
            log.debug("Could not register firewall rule (may require admin): {}", e.getMessage());
        }
    }

    // --- Connection handling ---

    /**
     * Handles an incoming TCP connection through the full receive protocol.
     * <p>
     * Protocol flow:
     * <ol>
     *   <li>Read {@code HANDSHAKE_INIT} — extract remote peer ID, nonce prefix, public key</li>
     *   <li>Rate-limit check against remote peer ID</li>
     *   <li>Respond with {@code HANDSHAKE_RESPONSE} containing local public key</li>
     *   <li>Derive ECDH shared secret and create AES-256-GCM encryption service</li>
     *   <li>Register connection in {@link ConnectionManager} and pin certificate</li>
     *   <li>Read {@code TRANSFER_REQUEST} — parse {@link FileMetadata}</li>
     *   <li>Check disk space; respond with {@code TRANSFER_ACCEPT} or {@code TRANSFER_REJECT}</li>
     *   <li>Receive chunk data, decrypt, decompress, verify hash, write to disk</li>
     *   <li>Send {@code CHUNK_ACK} or {@code CHUNK_NACK} per chunk</li>
     *   <li>On success: complete file writer, log audit entry</li>
     * </ol>
     *
     * @param socket the accepted TCP socket
     * @param reader the protocol message reader
     * @param writer the protocol message writer
     */
    private static void handleIncomingConnection(Socket socket, MessageReader reader, MessageWriter writer) {
        PeerId remotePeerId = null;
        try {
            // Step 1: Read HANDSHAKE_INIT
            MessageFrame initFrame = reader.readFrame();
            if (initFrame == null || initFrame.getType() != MessageType.HANDSHAKE_INIT) {
                writer.writeFrame(Messages.error("Expected HANDSHAKE_INIT"));
                return;
            }
            var pr = new PayloadReader(initFrame.getData());
            remotePeerId = PeerId.fromBytes(pr.readBytes());
            byte[] remoteNoncePrefix = pr.readBytes();
            byte[] remotePublicKey = pr.readBytes();

            if (!rateLimiter.allowRequest(remotePeerId.toHex())) {
                writer.writeFrame(Messages.error("Rate limit exceeded"));
                return;
            }

            // Step 2: Respond with our public key
            var keyGen = keyRotation.getKeyRotationManager().getCurrentKey();
            writer.writeFrame(Messages.handshakeResponse(localPeerId, keyGen.getPublicKey().getEncoded()));

            // Step 3: Derive shared secret — use initiator's nonce prefix
            KeyAgreement ka = KeyAgreement.getInstance("ECDH");
            ka.init(keyGen.getPrivateKey());
            KeyFactory kf = KeyFactory.getInstance("EC");
            kf.generatePublic(new X509EncodedKeySpec(remotePublicKey));
            ka.doPhase(kf.generatePublic(new X509EncodedKeySpec(remotePublicKey)), true);
            byte[] sharedSecret = ka.generateSecret();
            var encryptionKey = KeyDerivation.deriveAesKey(sharedSecret, "p2p-transfer-encryption");
            var encryption = new AesGcmEncryptionService(encryptionKey, remoteNoncePrefix);
            connectionManager.register(remotePeerId, socket, reader, writer);
            certPinning.pinKey(remotePeerId, remotePublicKey);

            // Step 4: Read TRANSFER_REQUEST
            MessageFrame transferFrame = reader.readFrame();
            if (transferFrame == null || transferFrame.getType() != MessageType.TRANSFER_REQUEST) {
                writer.writeFrame(Messages.error("Expected TRANSFER_REQUEST"));
                return;
            }
            var tpr = new PayloadReader(transferFrame.getData());
            String sessionId = tpr.readString();
            String fileName = tpr.readString();
            String relativePath = tpr.readString();
            long fileSize = tpr.readLong();
            String fileHash = tpr.readString();
            long lastModified = tpr.readLong();
            boolean isDirectory = tpr.readBoolean();
            boolean isCompressible = tpr.readBoolean();
            long totalChunks = tpr.readLong();
            long chunkSize = tpr.readLong();

            FileMetadata metadata = FileMetadata.builder()
                    .fileName(fileName).relativePath(relativePath).fileSize(fileSize)
                    .sha256Hash(fileHash).lastModified(lastModified).directory(isDirectory)
                    .compressible(isCompressible).chunkSize(chunkSize).build();

            if (!receiving) {
                log.debug("Rejecting transfer from {} — receive mode not enabled", remotePeerId.toShortString());
                writer.writeFrame(Messages.transferReject(sessionId, "Receiver is not accepting transfers"));
                return;
            }

            if (!isDirectory) {
                long freeSpace = receiveSaveDir.toFile().getFreeSpace();
                if (freeSpace < fileSize * 1.1) {
                    writer.writeFrame(Messages.transferReject(sessionId,
                            "Insufficient disk space: need " + (fileSize / 1024 / 1024) + "MB"));
                    return;
                }
            }

            writer.writeFrame(Messages.transferAccept(sessionId));
            var session = TransferSession.builder()
                    .sessionId(sessionId).localPeerId(localPeerId).remotePeerId(remotePeerId)
                    .fileMetadata(metadata).direction(TransferDirection.RECEIVE).build();
            activeTransfers.add(session);
            session.transitionTo(TransferState.HANDSHAKING);
            session.transitionTo(TransferState.NEGOTIATING);
            session.transitionTo(TransferState.TRANSFERRING);

            Path outputPath = receiveSaveDir.resolve(fileName);
            Files.createDirectories(outputPath.getParent());
            var compression = new DeflateCompressionService();
            boolean success = true;
            int failedChunks = 0;

            if (!isDirectory) {
                var fileWriter = new ChunkedFileWriter(outputPath, metadata);
                for (long i = 0; i < totalChunks && success; i++) {
                        MessageFrame chunkFrame = reader.readFrame();
                        if (chunkFrame == null) { success = false; break; }
                        if (chunkFrame.getType() == MessageType.CHUNK_DATA) {
                            var cpr = new PayloadReader(chunkFrame.getData());
                            cpr.readString(); // sessionId
                            long chunkIndex = cpr.readLong();
                            cpr.readLong(); // offset
                            int originalLength = cpr.readInt();
                            boolean compressed = cpr.readBoolean();
                            String chunkHash = cpr.readString();
                            byte[] encryptedData = cpr.readBytes();

                            try {
                                byte[] decrypted = encryption.decryptChunk(encryptedData, chunkIndex);
                                byte[] chunkData = compressed ? compression.decompress(decrypted) : decrypted;
                                if (fileWriter.writeChunk(chunkIndex, chunkData, chunkHash)) {
                                    writer.writeFrame(Messages.chunkAck(sessionId, chunkIndex));
                                    session.markChunkCompleted(chunkIndex, chunkData.length);
                                } else {
                                    writer.writeFrame(Messages.chunkNack(sessionId, chunkIndex, "Hash mismatch"));
                                    failedChunks++;
                                }
                            } catch (Exception e) {
                                writer.writeFrame(Messages.chunkNack(sessionId, chunkIndex, e.getMessage()));
                                failedChunks++;
                            }
                        }
                }

                if (success && failedChunks == 0) {
                    fileWriter.complete();
                    session.transitionTo(TransferState.VERIFYING);
                    session.transitionTo(TransferState.COMPLETED);
                    String actualHash = FileUtils.sha256(outputPath);
                    auditLogger.logTransfer(sessionId, actualHash, fileName, fileSize,
                            0L, remotePeerId.toHex(), localPeerId.toHex(), "COMPLETED", null);
                    String sizeStr = fileSize >= 1024*1024 ? String.format("%.1f MB", fileSize/(1024.0*1024))
                            : fileSize >= 1024 ? String.format("%.1f KB", fileSize/1024.0) : fileSize + " B";
                    System.out.printf("\nReceived '%s' (%s) from %s%n", fileName, sizeStr, remotePeerId.toShortString());
                } else {
                    fileWriter.abort();
                    session.transitionTo(TransferState.FAILED);
                    session.fail(failedChunks + " chunks failed");
                    auditLogger.logTransfer(sessionId, "", fileName, fileSize,
                            0L, remotePeerId.toHex(), localPeerId.toHex(), "FAILED",
                            failedChunks + " chunks failed");
                }
            }
        } catch (Exception e) {
            log.error("Handler error from {}: {}", socket.getRemoteSocketAddress(), e.getMessage(), e);
            String userMsg = e.getMessage();
            if (userMsg != null && userMsg.startsWith("Invalid state transition")) {
                userMsg = "Transfer already completed";
            }
            System.err.println("\nReceive error: " + (userMsg != null ? userMsg : "Unknown error"));
            try { writer.writeFrame(Messages.error("Handler error: " + e.getMessage())); } catch (Exception ignored) {}
        } finally {
            if (remotePeerId != null) {
                try { connectionManager.disconnect(remotePeerId); } catch (Exception ignored) {}
            }
        }
    }

    @Override
    public Integer call() {
        if (!interactiveMode) {
            CommandLine.usage(this, System.out);
            return 0;
        }
        return 0;
    }

    // --- Interactive shell ---

    /**
     * Launches the interactive shell REPL.
     * <p>
     * Displays a header banner and help text, then enters a read-eval-print loop
     * using {@link java.util.Scanner}. Each line is tokenized via {@link #tokenizeArgs}
     * and executed through a picocli {@link CommandLine} instance. Supports
     * {@code help}, {@code exit}/{@code quit}, and all subcommands. Calls
     * {@link #shutdown()} on exit.
     */
    static void interactiveShell() {
        System.out.println("╔══════════════════════════════════════════════╗");
        System.out.println("║    P2P File Transfer — Interactive Shell    ║");
        System.out.println("╚══════════════════════════════════════════════╝");
        System.out.println("Type a command and press Enter. Commands run live with");
        System.out.println("background services (discovery, TCP, health check).");
        System.out.println();
        System.out.println("  discover              Scan LAN for peers (5s wait)");
        System.out.println("  discover -t 10        Scan with custom timeout (sec)");
        System.out.println("  send <file> <peer>    Send file to peer by name/IP");
        System.out.println("  receive               Enable background file receiving");
        System.out.println("  status                Show connections & transfers");
        System.out.println("  config --show         View current configuration");
        System.out.println("  help                  Show this help");
        System.out.println("  exit / quit           Shutdown and exit");
        System.out.println();

        var cmd = new CommandLine(new P2PCli());

        try (java.util.Scanner scanner = new java.util.Scanner(System.in)) {
            while (true) {
                System.out.print(prompt());
                String line = scanner.nextLine().trim();
                if (line.isEmpty()) continue;

                if (line.equalsIgnoreCase("exit") || line.equalsIgnoreCase("quit")) {
                    System.out.println("Shutting down...");
                    break;
                }
                if (line.equalsIgnoreCase("help")) {
                    System.out.println("  discover [-t ms]       Scan LAN for peers");
                    System.out.println("  send <file> <peer>    Send file to peer");
                    System.out.println("  receive [--save-dir]  Listen for transfers");
                    System.out.println("  status                Show connections & transfers");
                    System.out.println("  config --show         View configuration");
                    System.out.println("  help / exit / quit    This help / exit");
                    continue;
                }

                try {
                    cmd.execute(tokenizeArgs(line));
                } catch (Exception e) {
                    System.err.println("Error: " + e.getMessage());
                }
            }
        }

        shutdown();
        System.out.println("Goodbye!");
    }

    /**
     * Splits a command line string into tokens, respecting double-quoted strings.
     *
     * @param line the raw input line
     * @return array of token strings
     */
    static String[] tokenizeArgs(String line) {
        var tokens = new java.util.ArrayList<String>();
        int i = 0;
        while (i < line.length()) {
            if (Character.isWhitespace(line.charAt(i))) { i++; continue; }
            if (line.charAt(i) == '"') {
                i++; int start = i;
                while (i < line.length() && line.charAt(i) != '"') i++;
                tokens.add(line.substring(start, i));
                if (i < line.length()) i++;
            } else {
                int start = i;
                while (i < line.length() && !Character.isWhitespace(line.charAt(i))) i++;
                tokens.add(line.substring(start, i));
            }
        }
        return tokens.toArray(new String[0]);
    }

    /**
     * Builds the interactive shell prompt string showing peer/connection/transfer counts.
     *
     * @return formatted prompt like {@code p2p [3P 2C 1T]> }
     */
    static String prompt() {
        int peers = discovery != null ? discovery.getDiscoveredPeers().size() : 0;
        int conns = connectionManager != null ? connectionManager.getActiveCount() : 0;
        int xfers = activeTransfers.size();
        return String.format("p2p [%dP %dC %dT]> ", peers, conns, xfers);
    }

    public static void main(String[] args) {
        initialize();
        if (args.length == 0) {
            interactiveMode = true;
            interactiveShell();
        } else {
            int exitCode = new CommandLine(new P2PCli()).execute(args);
            System.exit(exitCode);
        }
    }

    // --- Commands ---

    /**
     * Discovers LAN peers by listening for multicast announcements.
     * <p>
     * Waits up to the specified timeout for peer discovery responses,
     * then displays a formatted table of discovered peers.
     */
    @Command(name = "discover", description = "List discovered LAN peers")
    static class DiscoverCommand implements Callable<Integer> {
        @Option(names = {"--timeout", "-t"}, defaultValue = "5000",
                description = "Time to wait for peer responses in milliseconds (default: ${DEFAULT-VALUE})")
        long timeoutMs;

        @Override
        public Integer call() throws InterruptedException {
            // Discovery service is already running from initialize().
            // Wait for peer announcements to arrive over the network.
            if (timeoutMs > 0) {
                System.out.print("Discovering peers");
                long deadline = System.currentTimeMillis() + timeoutMs;
                int dots = 0;
                while (System.currentTimeMillis() < deadline) {
                    if (!discovery.getDiscoveredPeers().isEmpty()) {
                        System.out.println();
                        break;
                    }
                    if (dots < 3) {
                        System.out.print(".");
                        System.out.flush();
                        dots++;
                    }
                    Thread.sleep(Math.min(500, deadline - System.currentTimeMillis()));
                }
                if (dots > 0) System.out.println();
            }

            List<PeerInfo> peers = discovery.getDiscoveredPeers();
            if (peers.isEmpty()) {
                System.out.println("No peers discovered on LAN");
                return 0;
            }
            System.out.println("Discovered Peers:");
            System.out.println("──────────────────────────────────────────────────────────────");
            for (PeerInfo peer : peers) {
                System.out.printf("%-20s %-15s %-5d %-10s %s%n",
                        peer.getDisplayName(),
                        peer.getAddress().getHostAddress(),
                        peer.getPort(),
                        peer.getStatus(),
                        peer.getOperatingSystem());
            }
            return 0;
        }
    }

    /**
     * Sends a file or directory to a remote peer.
     * <p>
     * Protocol flow:
     * <ol>
     *   <li>Resolve target peer by display name or IP address</li>
     *   <li>Establish TCP connection with protocol framing</li>
     *   <li>Perform ECDH handshake: send {@code HANDSHAKE_INIT} with public key + nonce prefix,
     *       receive {@code HANDSHAKE_RESPONSE}, derive shared AES-256-GCM key</li>
     *   <li>Register connection in ConnectionManager and pin peer certificate</li>
     *   <li>Send {@code TRANSFER_REQUEST} with {@link FileMetadata}</li>
     *   <li>Await {@code TRANSFER_ACCEPT} (or reject with error message)</li>
     *   <li>Read file chunks, optionally compress with Deflate, encrypt, send as
     *       {@code CHUNK_DATA}, wait for {@code CHUNK_ACK}/{@code CHUNK_NACK} per chunk</li>
     *   <li>On NACK, retry the chunk</li>
     *   <li>Send {@code TRANSFER_COMPLETE} and log audit entry</li>
     * </ol>
     */
    @Command(name = "send", description = "Send file or directory to a peer")
    static class SendCommand implements Callable<Integer> {
        @Parameters(index = "0", description = "File or directory path to send")
        String path;

        @Parameters(index = "1", description = "Peer display name or IP")
        String peer;

        @Option(names = {"--priority"}, description = "Bump to front of transfer queue")
        boolean priority;

        @Option(names = {"--no-compress"}, description = "Disable compression")
        boolean noCompress;

        @Override
        public Integer call() {
            try {
                Path filePath = Paths.get(path);
                if (!Files.exists(filePath)) {
                    System.err.println("File not found: " + path);
                    return 1;
                }

                Optional<PeerInfo> target = discovery.findPeerByName(peer);
                if (target.isEmpty()) {
                    try {
                        InetAddress addr = InetAddress.getByName(peer);
                        for (PeerInfo p : discovery.getDiscoveredPeers()) {
                            if (p.getAddress().equals(addr)) {
                                target = Optional.of(p);
                                break;
                            }
                        }
                    } catch (Exception ignored) {}
                }
                if (target.isEmpty()) {
                    System.err.println("Peer not found: " + peer);
                    System.out.println("Run 'p2p discover' to find available peers");
                    return 1;
                }

                PeerInfo remotePeer = target.get();
                long fileSize = Files.isDirectory(filePath) ? 0 : Files.size(filePath);
                System.out.printf("Sending '%s' (%s) to %s (%s)...%n",
                        filePath.getFileName(), formatSize(fileSize),
                        remotePeer.getDisplayName(),
                        remotePeer.getAddress().getHostAddress());

                // Connect to peer
                var conn = tcpClient.connectWithProtocol(remotePeer.getAddress(), remotePeer.getPort());
                var socket = conn.socket();
                var reader = conn.reader();
                var writer = conn.writer();
                PeerId remotePeerId = remotePeer.getPeerId();
                boolean shouldCompress = !noCompress;

                try {
                    // Handshake: send our public key + nonce prefix
                    var keyGen = keyRotation.getKeyRotationManager().getCurrentKey();
                    byte[] ourNoncePrefix = KeyDerivation.generateNoncePrefix();
                    writer.writeFrame(Messages.handshakeInit(localPeerId, keyGen.getPublicKey().getEncoded(), ourNoncePrefix));

                    // Read handshake response
                    MessageFrame respFrame = reader.readFrame();
                    if (respFrame == null || respFrame.getType() != MessageType.HANDSHAKE_RESPONSE) {
                        throw new ProtocolException(ErrorCode.HANDSHAKE_FAILED, "Expected HANDSHAKE_RESPONSE");
                    }
                    var pr = new PayloadReader(respFrame.getData());
                    PeerId respPeerId = PeerId.fromBytes(pr.readBytes());
                    byte[] remotePublicKey = pr.readBytes();
                    if (!respPeerId.equals(remotePeerId)) {
                        throw new ProtocolException(ErrorCode.HANDSHAKE_FAILED, "Peer ID mismatch");
                    }

                    // Derive session key
                    KeyAgreement ka = KeyAgreement.getInstance("ECDH");
                    ka.init(keyGen.getPrivateKey());
                    KeyFactory kf = KeyFactory.getInstance("EC");
                    ka.doPhase(kf.generatePublic(new X509EncodedKeySpec(remotePublicKey)), true);
                    byte[] sharedSecret = ka.generateSecret();
                    var encryptionKey = KeyDerivation.deriveAesKey(sharedSecret, "p2p-transfer-encryption");
                    var encryption = new AesGcmEncryptionService(encryptionKey, ourNoncePrefix);
                    connectionManager.register(remotePeerId, socket, reader, writer);
                    certPinning.pinKey(remotePeerId, remotePublicKey);

                    // Send transfer request
                    String sessionId = UUID.randomUUID().toString().replace("-", "");
                    FileMetadata metadata;
                    if (Files.isDirectory(filePath)) {
                        long dirSize = FileUtils.totalSize(filePath);
                        metadata = FileMetadata.builder()
                                .fileName(filePath.getFileName().toString())
                                .relativePath("")
                                .fileSize(dirSize).directory(true).chunkSize(config.getChunkSize()).build();
                    } else {
                        metadata = FileMetadata.builder()
                                .fileName(filePath.getFileName().toString())
                                .relativePath("")
                                .fileSize(Files.size(filePath))
                                .sha256Hash(FileUtils.sha256(filePath))
                                .lastModified(filePath.toFile().lastModified())
                                .directory(false)
                                .compressible(FileMetadata.isCompressible(filePath))
                                .chunkSize(config.getChunkSize()).build();
                    }

                    writer.writeFrame(Messages.transferRequest(sessionId, metadata));
                    MessageFrame acceptFrame = reader.readFrame();
                    if (acceptFrame == null) {
                        throw new ProtocolException(ErrorCode.CONNECTION_LOST, "Connection closed during negotiation");
                    }
                    if (acceptFrame.getType() == MessageType.TRANSFER_REJECT) {
                        var rpr = new PayloadReader(acceptFrame.getData());
                        rpr.readString(); // sessionId
                        throw new TransferException(ErrorCode.TRANSFER_REJECTED, "Rejected: " + rpr.readString());
                    }
                    if (acceptFrame.getType() != MessageType.TRANSFER_ACCEPT) {
                        throw new ProtocolException(ErrorCode.UNEXPECTED_MESSAGE, "Expected TRANSFER_ACCEPT");
                    }

                    var session = TransferSession.builder()
                            .sessionId(sessionId).localPeerId(localPeerId).remotePeerId(remotePeerId)
                            .fileMetadata(metadata).direction(TransferDirection.SEND).build();
                    activeTransfers.add(session);
                    session.transitionTo(TransferState.HANDSHAKING);
                    session.transitionTo(TransferState.NEGOTIATING);
                    session.transitionTo(TransferState.TRANSFERRING);

                    // Send chunks
                    var compression = new DeflateCompressionService();
                    if (!Files.isDirectory(filePath)) {
                        try (var fileReader = new ChunkedFileReader(filePath, metadata)) {
                            long totalChunks = metadata.getTotalChunks();
                            System.out.printf("  0/%d chunks (0%%)", totalChunks);
                            for (long i = 0; i < totalChunks; i++) {
                                byte[] chunkData = fileReader.readChunk(i);
                                byte[] dataToSend;
                                boolean compressed = false;
                                if (shouldCompress && metadata.isCompressible() && chunkData.length > 1024) {
                                    byte[] comp = compression.compress(chunkData);
                                    if (comp.length < chunkData.length) {
                                        dataToSend = comp;
                                        compressed = true;
                                    } else {
                                        dataToSend = chunkData;
                                    }
                                } else {
                                    dataToSend = chunkData;
                                }
                                String chunkHash = FileUtils.sha256(dataToSend);
                                byte[] encrypted = encryption.encryptChunk(dataToSend, i);
                                writer.writeFrame(Messages.chunkData(sessionId, i,
                                        i * metadata.getChunkSize(), chunkData.length,
                                        compressed, chunkHash, encrypted));

                                MessageFrame ackFrame = reader.readFrame();
                                if (ackFrame == null) {
                                    throw new ProtocolException(ErrorCode.CONNECTION_LOST, "Connection lost waiting for ack");
                                }
                                if (ackFrame.getType() == MessageType.CHUNK_NACK) {
                                    var npr = new PayloadReader(ackFrame.getData());
                                    npr.readString(); npr.readLong(); String reason = npr.readString();
                                    log.warn("Chunk {} rejected ({}), retrying", i, reason);
                                    i--;
                                    continue;
                                }
                                session.markChunkCompleted(i, chunkData.length);
                                int pct = (int) ((i + 1) * 100 / totalChunks);
                                System.out.printf("\r  %d/%d chunks (%d%%)", i + 1, totalChunks, pct);
                            }
                            System.out.println();
                        }
                    }

                    // Complete
                    String finalHash = Files.isDirectory(filePath) ? "" : FileUtils.sha256(filePath);
                    writer.writeFrame(Messages.transferComplete(sessionId, finalHash));
                    session.transitionTo(TransferState.COMPLETED);
                    auditLogger.logTransfer(sessionId, "", metadata.getFileName(), metadata.getFileSize(),
                            0L, localPeerId.toHex(), remotePeerId.toHex(), "COMPLETED", null);
                    System.out.println("Transfer completed successfully");

                } finally {
                    conn.close();
                }
                return 0;
            } catch (Exception e) {
                String msg = e.getMessage();
                if (msg != null && msg.startsWith("Invalid state transition")) {
                    msg = "Transfer already completed";
                }
                System.err.println(msg != null ? msg : "Send failed due to an unexpected error");
                return 1;
            }
        }
    }

    /**
     * Listens for incoming file transfers from peers.
     * <p>
     * Enables the receiver flag and optionally sets a custom save directory.
     * In interactive mode, this runs as a background task; in CLI mode it
     * blocks indefinitely until Ctrl+C.
     */
    @Command(name = "receive", description = "Listen for incoming transfers")
    static class ReceiveCommand implements Callable<Integer> {
        @Option(names = {"--accept-all"}, description = "Auto-accept all incoming transfers")
        boolean acceptAll;

        @Option(names = {"--save-dir"}, description = "Save directory (default: ~/Desktop/downloads)")
        String saveDir;

        @Override
        public Integer call() {
            receiving = true;
            Path dir = saveDir != null ? Paths.get(saveDir) : Paths.get(System.getProperty("user.home"), "Desktop", "downloads");
            receiveSaveDir = dir;
            try {
                Files.createDirectories(dir);
            } catch (IOException e) {
                System.err.println("Failed to create save directory: " + e.getMessage());
                return 1;
            }
            System.out.println("Listening for incoming transfers...");
            System.out.println("Save directory: " + dir.toAbsolutePath());
            System.out.println("Auto-accept: " + (acceptAll ? "yes" : "no"));
            if (interactiveMode) {
                System.out.println("Receiving enabled in background. Type 'status' to check transfers.");
                return 0;
            }
            System.out.println("Press Ctrl+C to stop");
            try {
                Thread.sleep(Long.MAX_VALUE);
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
            }
            return 0;
        }
    }

    /**
     * Displays current system status including peer ID, platform, connections,
     * discovered peers, and active transfers.
     */
    @Command(name = "status", description = "Show transfer status")
    static class StatusCommand implements Callable<Integer> {
        @Override
        public Integer call() {
            System.out.println("P2P Transfer Status");
            System.out.println("──────────────────────────────────────────────");
            System.out.println("Peer ID: " + localPeerId.toShortString());
            System.out.println("Platform: " + PlatformUtils.getPlatformString());
            System.out.println("TCP Port: " + config.getTcpPort());
            System.out.println("Discovery: " + (discovery != null && discovery.isRunning() ? "running" : "stopped"));
            System.out.println("Active Connections: " + (connectionManager != null ? connectionManager.getActiveCount() : 0));
            System.out.println("Discovered Peers: " + (discovery != null ? discovery.getDiscoveredPeers().size() : 0));
            System.out.println("Active Transfers: " + activeTransfers.size());
            if (!activeTransfers.isEmpty()) {
                System.out.println();
                for (var s : activeTransfers) {
                    var p = s.getProgress();
                    System.out.printf("  %s → %s: %s (%d%%)\n",
                            s.getDirection() == TransferDirection.SEND ? "Local" : s.getRemotePeerId().toShortString(),
                            s.getDirection() == TransferDirection.RECEIVE ? "Local" : s.getRemotePeerId().toShortString(),
                            s.getState(),
                            (int)p.percentComplete());
                }
            }
            return 0;
        }
    }

    /**
     * Views the current runtime configuration.
     * <p>
     * Displays all configurable parameters including ports, chunk size,
     * parallelism, encryption/compression flags, data directory, and display name.
     */
    @Command(name = "config", description = "View or update configuration")
    static class ConfigCommand implements Callable<Integer> {
        @Option(names = {"--show"}, description = "Show all configuration")
        boolean show;

        @Option(names = {"--set"}, description = "Set config key=value (e.g. tcpPort=9877)")
        String set;

        @Override
        public Integer call() {
            if (set != null && !set.isEmpty()) {
                System.out.println("Runtime config changes not persisted yet");
                return 0;
            }
            System.out.println("Current Configuration:");
            System.out.println("──────────────────────────────────");
            System.out.printf("%-25s %s%n", "tcpPort:", config.getTcpPort());
            System.out.printf("%-25s %s%n", "discoveryPort:", config.getDiscoveryPort());
            System.out.printf("%-25s %s%n", "multicastGroup:", config.getMulticastGroup());
            System.out.printf("%-25s %s%n", "chunkSize:", formatSize(config.getChunkSize()));
            System.out.printf("%-25s %s%n", "parallelism:", config.getParallelism());
            System.out.printf("%-25s %s%n", "maxParallelism:", config.getMaxParallelism());
            System.out.printf("%-25s %s%n", "encryption:", config.isEncryptionEnabled());
            System.out.printf("%-25s %s%n", "compression:", config.isCompressionEnabled());
            System.out.printf("%-25s %s%n", "dataDirectory:", config.getDataDirectory());
            System.out.printf("%-25s %s%n", "displayName:", config.getDisplayName());
            return 0;
        }
    }

    /**
     * Displays version and build information.
     */
    @Command(name = "version", description = "Show version information")
    static class VersionCommand implements Callable<Integer> {
        @Override
        public Integer call() {
            System.out.println(new VersionProvider().getVersion()[0]);
            return 0;
        }
    }

    /**
     * Provides version strings for picocli's {@code --version} help option.
     */
    static class VersionProvider implements CommandLine.IVersionProvider {
        @Override
        public String[] getVersion() {
            return new String[] {
                "P2P File Transfer System v1.0.0-SNAPSHOT",
                "Platform: " + PlatformUtils.getPlatformString(),
                "Java: " + System.getProperty("java.version"),
                "Peer ID: " + localPeerId.toShortString()
            };
        }
    }

    /**
     * Formats a byte count into a human-readable string (B, KB, MB, GB).
     *
     * @param bytes the size in bytes
     * @return formatted string like "4.2 MB"
     */
    static String formatSize(long bytes) {
        if (bytes < 1024) return bytes + " B";
        if (bytes < 1024 * 1024) return String.format("%.1f KB", bytes / 1024.0);
        if (bytes < 1024 * 1024 * 1024) return String.format("%.1f MB", bytes / (1024.0 * 1024));
        return String.format("%.2f GB", bytes / (1024.0 * 1024 * 1024));
    }
}
