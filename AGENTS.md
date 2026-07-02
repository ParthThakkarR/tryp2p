# P2P File Transfer System — Agent Reference

## Project Structure
```
p2p/
├── settings.gradle.kts              # Multi-module Gradle config
├── build.gradle.kts                  # Root build: Java 21, JUnit Platform
├── gradle/libs.versions.toml         # Version catalog
├── p2p-core/                         # Domain models, interfaces, utilities
├── p2p-network/                      # TCP/UDP networking, discovery, protocol
├── p2p-crypto/                       # Encryption, hashing, key exchange
├── p2p-transfer/                     # Transfer engine, chunking, compression, resume
├── p2p-security/                     # Rate limiting, cert pinning, auth logging, firewall
├── p2p-observability/                # Metrics, structured logging, health, audit
├── p2p-relay/                        # STUN, NAT traversal, global registry client, relay
├── p2p-cli/                          # CLI interface (picocli)
├── p2p-app/                          # Application bootstrap, DI wiring
└── registry-server/                  # Docker setup for registry & relay servers
```

## Module Dependency Graph
```
p2p-core → p2p-network → p2p-transfer → p2p-cli → p2p-app
p2p-core → p2p-crypto  → p2p-transfer
p2p-core → p2p-crypto  → p2p-security → p2p-cli
p2p-core → p2p-observability → p2p-cli
p2p-core → p2p-network  → p2p-relay → p2p-cli
```

## Build & Test Commands
```bash
./gradlew build              # Compile all modules (verified on JDK 26 + Gradle 9.6.1)
./gradlew test               # Run all unit tests (105 passing)
./gradlew clean build        # Clean build
./gradlew shadowJar          # Build fat JAR → p2p-app/build/libs/p2p-1.0.0-SNAPSHOT.jar
```

### Cross-Platform Packaging
```bash
# Windows (run on Windows)
./gradlew :p2p-app:packageWinApp       # Portable .exe + bundled JRE
./gradlew :p2p-app:packageWinPortable  # Single-file SFX EXE (requires 7-Zip)
./gradlew :p2p-app:packageWinZip       # ZIP distribution (~55MB)
./gradlew :p2p-app:packageWinMsi       # MSI installer (requires WiX Toolset)

# macOS (run on macOS)
./gradlew :p2p-app:packageMacApp   # Portable .app bundle
./gradlew :p2p-app:packageMacDmg   # DMG installer

# Linux (run on Linux)
./gradlew :p2p-app:packageLinuxApp # Portable app image
./gradlew :p2p-app:packageLinuxDeb # DEB package (Debian/Ubuntu)

# One-step platform-aware build
.\build-installer.ps1 -Target portable # Windows: single-file portable SFX EXE
.\build-installer.ps1 -Target zip      # Windows: clean build + test + ZIP
./build-package.sh                     # Linux/Mac: auto-detect OS, build native package
./build-package.sh --test              # Linux/Mac: with tests first
```

### Universal (no Java required, any OS)
```bash
# Just the JAR — run anywhere Java 21+ is installed
java -jar p2p-app/build/libs/p2p-1.0.0-SNAPSHOT.jar --gui
```

## Build Status ✅
- **JDK**: 26.0.1 (Oracle)
- **Gradle**: 9.6.1 (bundles Kotlin 2.1+ — supports Java 26)
- **Shadow plugin**: 9.4.3 (id: `com.gradleup.shadow`)
- **All 9 modules compile**: p2p-core, p2p-crypto, p2p-network, p2p-observability, p2p-security, p2p-transfer, p2p-relay, p2p-cli, p2p-app
- **All tests pass**: p2p-core (70 tests), p2p-crypto (10 tests), p2p-network (17 tests), p2p-transfer (8 tests) — all green

## Key Packages

### p2p-core
- `com.p2p.core.model` — PeerId, PeerInfo, PeerStatus, FileMetadata, ChunkInfo, TransferSession, TransferState, TransferProgress, TransferDirection
- `com.p2p.core.event` — EventBus, P2PEvent, EventListener
- `com.p2p.core.service` — TransferService, PeerDiscoveryService, EncryptionService, CompressionService
- `com.p2p.core.config` — AppConfig (builder pattern, all defaults)
- `com.p2p.core.util` — PlatformUtils, NetworkUtils, FileUtils
- `com.p2p.core.exception` — P2PException, ErrorCode, NetworkException, ProtocolException, CryptoException, TransferException, SecurityException

### p2p-network
- `com.p2p.network.protocol` — MessageFrame, MessageType, Messages, PayloadBuilder, PayloadReader, MessageReader, MessageWriter
- `com.p2p.network.tcp` — TcpServer, TcpClient, ConnectionManager (with heartbeats)
- `com.p2p.network.discovery` — MulticastDiscoveryService

### p2p-crypto
- `com.p2p.crypto` — AesGcmEncryptionService, ECDHKeyExchange, KeyDerivation, KeyRotationManager

### p2p-transfer
- `com.p2p.transfer.engine` — TransferEngine, ChunkedFileReader, ChunkedFileWriter, ParallelTransferEngine, AdaptiveChunkSizer
- `com.p2p.transfer.compression` — DeflateCompressionService
- `com.p2p.transfer.resume` — TransferResumeManager

### p2p-security
- `com.p2p.security.ratelimit` — RateLimiter (token bucket per-peer)
- `com.p2p.security.pinning` — CertificatePinningManager (TOFU-based)
- `com.p2p.security.authlog` — PeerAuthenticationLog
- `com.p2p.security.credential` — CredentialStore, CredentialStoreFactory
- `com.p2p.security.firewall` — FirewallRuleGenerator
- `com.p2p.security.keyrotation` — KeyRotationScheduler

### p2p-observability
- `com.p2p.observability.metrics` — MetricsExporter (Prometheus)
- `com.p2p.observability.health` — HealthCheckServer
- `com.p2p.observability.audit` — TransferAuditLogger
- `com.p2p.observability.errors` — ErrorReporter, SentryErrorReporter

### p2p-cli
- `com.p2p.cli` — P2PCli (main CLI), DiscoverCommand, SendCommand, ReceiveCommand, StatusCommand, ConfigCommand

## Domain Model Key Facts
- **PeerId**: 16-byte random, Comparable, hex string
- **TransferSession**: Thread-safe via synchronized, state machine validation
- **FileMetadata**: Immutable, supports directory scan
- **ChunkInfo**: Immutable, tracks compression status
- **TransferState**: PENDING→HANDSHAKING→NEGOTIATING→TRANSFERRING→VERIFYING→COMPLETED, plus PAUSED/INTERRUPTED/FAILED/CANCELLED
- **AppConfig**: CLI > env > file > defaults

## Protocol Format
- Magic: `P2PF` (4B), Version: `0x01` (1B), Length: uint32 (4B), Type: byte (1B), Payload: variable
- Max payload: 16MB
- Message types: 14 types (DISCOVERY_ANNOUNCE through ERROR)

## CLI Commands
- `p2p discover` — List discovered LAN peers
- `p2p send <path> <peer>` — Send file/directory to a peer (name or IP)
- `p2p receive` — Listen for incoming transfers (Ctrl+C to stop)
- `p2p status` — Show peer ID, connections, active transfers
- `p2p config --show` — View configuration
- `p2p version` — Show version information

## Key API Signatures

### Crypto (`p2p-crypto`)
```
KeyRotationManager.getCurrentKey() → KeyGeneration
  KeyGeneration.getPublicKey() → PublicKey   (use .getEncoded() for bytes)
  KeyGeneration.getPrivateKey() → PrivateKey

ECDHKeyExchange()                    // generates ephemeral keypair
  .getPublicKeyBytes() → byte[]
  .deriveSharedSecret(byte[] remotePublicKeyBytes) → byte[]
  static computeFingerprint(byte[] publicKeyBytes) → String

KeyDerivation.deriveAesKey(byte[] sharedSecret, String info) → SecretKey
KeyDerivation.generateNoncePrefix() → byte[4]
KeyDerivation.buildNonce(byte[] prefix, long counter) → byte[12]

AesGcmEncryptionService(SecretKey key, byte[] noncePrefix)
  .encryptChunk(byte[] plaintext, long chunkIndex) → byte[]
  .decryptChunk(byte[] ciphertext, long chunkIndex) → byte[]
```

### Key Agreement (use directly with KeyRotation keys)
```java
KeyAgreement ka = KeyAgreement.getInstance("ECDH");
ka.init(privateKey);
KeyFactory kf = KeyFactory.getInstance("EC");
ka.doPhase(kf.generatePublic(new X509EncodedKeySpec(remotePubKeyBytes)), true);
byte[] sharedSecret = ka.generateSecret();
```

### Transfer (`p2p-transfer`)
```
ChunkedFileReader(Path, FileMetadata)
  .readChunk(long chunkIndex) → byte[]
  .readChunkWithHash(long chunkIndex) → ChunkData(info, data)

ChunkedFileWriter(Path, FileMetadata)
  .writeChunk(long chunkIndex, byte[] data, String expectedHash) → boolean
  .complete()
  .abort()
  .getWrittenChunks() → ConcurrentSkipListSet<Long>

TransferEngine(int maxParallelism, int maxInflightChunks)
  .createMetadata(Path, long chunkSize) → FileMetadata
  .prepareChunk(ChunkedFileReader, long chunkIndex, ...) → PreparedChunk
  .processReceivedChunk(ChunkedFileWriter, long chunkIndex, ...) → boolean

ParallelTransferEngine(AppConfig)
  .sendFile(Path, PeerInfo, ...) → TransferResult
  .receiveFile(ChunkedFileWriter, FileMetadata, ...) → TransferResult

DeflateCompressionService()
  .compress(byte[]) → byte[]
  .decompress(byte[]) → byte[]
```

### Protocol Messages (`p2p-network`)
```
Messages.handshakeInit(PeerId, byte[] publicKey) → MessageFrame
Messages.handshakeResponse(PeerId, byte[] publicKey) → MessageFrame
Messages.transferRequest(String sessionId, FileMetadata metadata) → MessageFrame
Messages.transferAccept(String sessionId) → MessageFrame
Messages.transferReject(String sessionId, String reason) → MessageFrame
Messages.transferComplete(String sessionId, String fileHash) → MessageFrame
Messages.chunkData(sessionId, chunkIndex, offset, originalLength, compressed, hash, data) → MessageFrame
Messages.chunkAck(sessionId, chunkIndex) → MessageFrame
Messages.chunkNack(sessionId, chunkIndex, reason) → MessageFrame
Messages.error(String) → MessageFrame
```

### Networking
```
TcpServer(AppConfig, ConnectionHandler)   // handler: (Socket, MessageReader, MessageWriter) → void
TcpClient()
  .connect(InetAddress, int) → Socket
  .connectWithProtocol(InetAddress, int) → Connection(socket, reader, writer, AutoCloseable)

ConnectionManager(heartbeatIntervalMs, missedThreshold)
  .register(PeerId, Socket, MessageReader, MessageWriter)
  .getConnection(PeerId) → Optional<ManagedConnection>
  .disconnect(PeerId)                    // unregisters + closes socket

PayloadReader(byte[])
  .readString() → String  .readBytes() → byte[]  .readByte() → byte
  .readInt() → int  .readLong() → long  .readBoolean() → boolean

PayloadBuilder()
  .writeString(String)  .writeBytes(byte[])  .writeByte(byte)
  .writeInt(int)  .writeLong(long)  .writeBoolean(boolean)  .build() → byte[]
```

### Audit
```
TransferAuditLogger(Path dataDir, int retentionDays)
  .logTransfer(AuditEntry)
  .logTransfer(transferId, fileHash, fileName, fileSize, durationMs,
               sourcePeer, destPeer, status, errorMessage)
```

### RateLimiter
```
RateLimiter(int maxRequestsPerMinute, Duration blockDuration, Path dataDir)
  .allowRequest(String peerId) → boolean
  .isBlocked(String peerId) → boolean
  .blockPeer(String peerId)  .unblockPeer(String peerId)
```

### p2p-relay
- `com.p2p.relay` — StunClient (RFC 8489), RegistryClient (HTTP), NatTraversalService, RegistryServer (standalone), RelayServer, RelayClient

## Global Connectivity
1. **LAN**: UDP multicast discovery (239.255.80.50:9876)
2. **Internet**: STUN + registry → direct TCP → hole punch → relay fallback
3. **Registry**: HTTP REST API, peers register public IP:port, Docker deployable
4. **STUN**: Google's stun.l.google.com:19302 for public address discovery
5. **Relay**: Simple TCP stream bridge for when NAT traversal fails
6. **CLI `send`**: Falls back from LAN discovery → registry lookup by name/IP

## Key Design Decisions
- Java 21+ virtual threads for concurrency
- AES-256-GCM chunked AEAD with unique per-chunk nonces (4B prefix + 8B counter)
- ECDH secp256r1 for key exchange
- HKDF for key derivation from shared secret
- Deflate with adaptive estimation (skip already-compressed formats)
- Chunk size: 512KB–2MB adaptive based on RTT
- Parallelism: 4 default, max 16
- Heartbeat: 5s interval, 2 missed → disconnect (~15s detection)
- Multicast: 239.255.80.50:9876
- TCP port: 9877
- Rate limit: 10 discovery/min, 5 connections/min per peer
- Key rotation: 7 days with 1-hour fallback window
- Audit retention: 90 days
- Resume expiry: 7 days

## P2PCli Module Notes
- All service wiring lives in `P2PCli.initialize()` (static fields)
- `handleIncomingConnection()` is the TCP connection handler lambda wired into `TcpServer`
- `SendCommand.call()` uses `tcpClient.connectWithProtocol()` → raw socket I/O (not ParallelTransferEngine)
- Handshake uses `KeyRotationManager.getCurrentKey()` for identity keypair, `KeyAgreement` directly for ECDH
- Channel encryption uses `AesGcmEncryptionService(SecretKey, byte[])` with `KeyDerivation.deriveAesKey()` and `KeyDerivation.generateNoncePrefix()`
- Unlimited file sizes: all chunk index fields are `long`, `FileMetadata.totalChunks` returns `long`
- Transfer state machine: `TransferSession.builder()` with `transitionTo()`
