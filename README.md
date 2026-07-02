# P2P File Transfer System

Cross-platform peer-to-peer file transfer system with encryption, compression, and LAN discovery.

## Quick Start

```bash
# Build
./gradlew build

# Run (CLI mode - no args shows help)
./gradlew :p2p-app:run

# Run via fat JAR
./gradlew shadowJar
java --enable-preview -jar p2p-app/build/libs/p2p-1.0.0-SNAPSHOT.jar
```

## CLI Commands

| Command | Description |
|---------|-------------|
| `p2p discover` | List discovered LAN peers |
| `p2p send <file> <peer>` | Send file to a peer |
| `p2p receive` | Listen for incoming transfers |
| `p2p status` | Show transfer status |
| `p2p config --show` | View configuration |
| `p2p version` | Show version info |

## Architecture

```
p2p-core/        Domain models, interfaces, utilities
p2p-network/     TCP/UDP networking, LAN discovery, binary protocol
p2p-crypto/      AES-256-GCM encryption, ECDH key exchange, key rotation
p2p-transfer/    Transfer engine, chunking, compression, resume
p2p-security/    Rate limiting, cert pinning, auth logging, firewall
p2p-observability/  Prometheus metrics, health checks, audit logging
p2p-cli/         CLI interface (picocli)
p2p-app/         Application bootstrap
```

## Key Features

- **Unlimited file sizes** - No artificial limits; only disk space matters
- **AES-256-GCM encryption** - Authenticated encryption per chunk
- **ECDH key exchange** - Forward secrecy, no pre-shared keys
- **Adaptive compression** - Deflate with ratio estimation
- **Parallel transfer** - Virtual threads with adaptive chunk sizing (512KB-2MB)
- **Transfer resume** - Automatic recovery from interruptions
- **Directory transfer** - Recursive directory structure preservation
- **LAN discovery** - UDP multicast auto-discovery
- **Key rotation** - 7-day automatic rotation with 1-hour fallback
- **Rate limiting** - Per-peer token bucket DDoS protection
- **Certificate pinning** - TOFU-based peer identity verification
- **Prometheus metrics** - /metrics endpoint on port 9090
- **Health checks** - /health endpoint on port 9091

## Protocol

Binary protocol over TCP: `[P2PF:4B][Version:1B][Length:4B][Type:1B][Payload:N]`

- Max payload: 16MB per message
- 14 message types for discovery, handshake, transfer, resume
- Chunked streaming for unlimited file sizes

## Building Packages

```bash
# Docker
docker build -t p2p-transfer .

# Windows MSI (requires jpackage)
./gradlew jpackage

# Linux DEB
./gradlew jpackage -PpackageType=deb
```

## Configuration

Precedence: CLI flags > environment variables > config file > defaults

Key defaults:
- TCP port: 9877
- Discovery: 239.255.80.50:9876 (UDP multicast)
- Chunk size: 1MB (adaptive 512KB-2MB)
- Parallelism: 4 (max 16)
- Key rotation: 7 days
- Audit retention: 90 days
