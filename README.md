# P2P File Transfer System

**Secure, direct peer-to-peer file transfers — across the room or across the world.**

Transfer files and folders between any devices (Windows, macOS, Linux) without cloud storage.
Direct device-to-device with AES-256-GCM encryption and adaptive compression.

## Features

- **🌍 Global & LAN** — UDP multicast for local peers, HTTP registry + STUN for internet peers
- **🔒 Encrypted** — AES-256-GCM per-chunk AEAD with ECDH key exchange (forward secrecy)
- **📁 Unlimited sizes** — Chunked streaming, only disk space limits
- **⚡ Fast** — Parallel virtual threads, adaptive 512KB–2MB chunks, Deflate compression
- **🔄 Resume** — Automatic recovery from interruptions
- **📂 Directories** — Recursive directory transfer with structure preservation
- **🛡️ Secure** — Rate limiting, certificate pinning, key rotation, firewall rules
- **📊 Observable** — Prometheus metrics, health checks, structured audit logging

## Quick Start

```bash
# Build everything
./gradlew build

# Run CLI (interactive shell)
java -jar p2p-app/build/libs/p2p-1.0.0-SNAPSHOT.jar --cli

# Run GUI
java -jar p2p-app/build/libs/p2p-1.0.0-SNAPSHOT.jar    # GUI is default
```

### Windows

| Package | File | How to use |
|---------|------|------------|
| **Portable EXE** | `p2p-app/build/dist-portable/*.exe` | Run directly, no install |
| **MSI Installer** | `p2p-app/build/dist-msi/*.msi` | Install with start menu + uninstall |

## CLI Commands

| Command | Description |
|---------|-------------|
| `p2p discover` | Discover LAN + internet peers |
| `p2p send <file> <peer>` | Send file/directory to peer by name or IP |
| `p2p receive` | Enable background file receiving |
| `p2p status` | Show connections and active transfers |
| `p2p config --show` | View configuration |
| `p2p version` | Show version info |

### Example

```bash
# Start receiving
p2p receive

# In another terminal — discover peers
p2p discover

# Send a file
p2p send report.pdf alice
```

## Global Connectivity

Peers can connect across the internet using:

1. **Direct TCP** — If both peers have public IPs or port forwarding
2. **STUN** — Automatically discovers your public IP:port using Google's STUN server
3. **TCP hole punching** — Simultaneous open for NAT traversal
4. **Relay fallback** — TCP relay server when direct connection fails

Peers register with a central registry server so others can find them globally.

### Running your own registry server

```bash
# Start registry + relay
cd registry-server
docker-compose up -d

# Configure your app
p2p config --set registryUrl=http://your-server:8080
p2p config --set relayHost=your-server
```

## Architecture

```
p2p-core/           Domain models, interfaces, utilities
p2p-network/        TCP/UDP networking, LAN discovery, binary protocol
p2p-crypto/         AES-256-GCM, ECDH key exchange, key rotation
p2p-transfer/       Transfer engine, chunking, compression, resume
p2p-security/       Rate limiting, cert pinning, auth logging, firewall
p2p-observability/  Prometheus metrics, health checks, audit logging
p2p-relay/          STUN, registry client, NAT traversal, relay
p2p-cli/            CLI interface (picocli)
p2p-app/            Application bootstrap and packaging
```

### Protocol

Binary protocol over TCP: `[P2PF:4B][Version:1B][Length:4B][Type:1B][Payload:N]`

- Max payload: 16MB per message
- 14 message types for discovery, handshake, transfer, resume
- Chunked streaming for unlimited file sizes

## Building Packages

### Windows (on Windows)

```powershell
.\gradlew :p2p-app:packageWinPortable   # Single-file portable EXE
.\gradlew :p2p-app:packageWinMsi        # MSI installer
.\gradlew :p2p-app:packageWinZip        # ZIP distribution
```

### macOS (on macOS)

```bash
./gradlew :p2p-app:packageMacApp  # .app bundle
./gradlew :p2p-app:packageMacDmg  # DMG installer
```

### Linux (on Linux)

```bash
./gradlew :p2p-app:packageLinuxApp  # Portable app image
./gradlew :p2p-app:packageLinuxDeb  # DEB package
```

## Configuration

Precedence: CLI flags > environment variables > config file > defaults

Key defaults:
| Setting | Default | Description |
|---------|---------|-------------|
| TCP port | 9877 | File transfer port |
| Discovery port | 9876 | UDP multicast discovery |
| Multicast group | 239.255.80.50 | LAN discovery address |
| Chunk size | 1MB | Adaptive 512KB-2MB |
| Parallelism | 4 | Max 16 concurrent transfers |
| Encryption | enabled | AES-256-GCM |
| Compression | enabled | Deflate with ratio check |
| Registry URL | https://registry.p2ptransfer.io | Global peer registry |
| Relay host | relay.p2ptransfer.io | TCP relay fallback |
| Key rotation | 7 days | With 1-hour overlap |
| Audit retention | 90 days | Transfer history |
