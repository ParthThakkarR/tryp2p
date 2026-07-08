# p2ptransfer

**Secure, direct peer-to-peer file transfers — across the room or across the world.**

Transfer files between any devices (Windows, macOS, Linux) without cloud storage.
Direct device-to-device with ChaCha20-Poly1305 encryption, ECDH key exchange,
adaptive Zstd compression, and automatic resume.

## Features

- **LAN Discovery** — UDP multicast, zero-config peer discovery
- **Encrypted** — Ephemeral ECDH per session + ChaCha20-Poly1305 AEAD per chunk
- **Unlimited sizes** — Chunked streaming, only disk space limits
- **Fast** — Adaptive Zstd compression, skip already-compressed formats
- **Resume** — Automatic recovery from interruptions via SQLite tracking
- **Error Recovery** — Exponential backoff retry with reconnect + re-handshake
- **STUN** — Discover public IP for NAT traversal
- **TCP Relay** — Fallback relay for when direct connection fails
- **NAT Hole Punching** — Simultaneous TCP open for NAT traversal

## Quick Start

```bash
# CLI — send a file
p2p send myfile.pdf 192.168.1.100:9877

# CLI — listen for incoming transfers
p2p listen

# CLI — discover LAN peers
p2p list

# GUI — desktop application
cargo run -p p2ptransfer-gui
```

## Package

| Platform | Format | Command |
|----------|--------|---------|
| Windows | MSI | `wix\build-wix.bat` |
| macOS | DMG | `scripts\package-macos.sh` |
| Linux | DEB | `scripts\package-linux.sh` |

## Architecture

```
p2ptransfer-core/    Core: P2P, crypto, compression, transfer
├── compress/       Zstd + LZ4 + format detection
├── crypto/         ECDH (x25519-dalek) + ChaCha20-Poly1305
├── network/        TCP, STUN, relay, NAT hole punch
├── p2p/            UDP multicast discovery, peer management
├── storage/        File iteration
└── transfer/       Engine, chunker, hasher, resume (SQLite)
desktop/
├── p2ptransfer-cli/ CLI (clap + TUI progress)
└── p2ptransfer-gui/ Tauri v1 + React
mobile/
├── p2ptransfer-ffi/  Raw FFI cdylib
└── p2ptransfer-flutter/ Flutter + flutter_rust_bridge
```

## Protocol

1. ECDH handshake (CLIENT_HELLO / SERVER_HELLO)
2. Metadata exchange (file info, checksum, nonce prefix)
3. Encrypted chunk transfer with ACKs
4. Final verification (BLAKE3 checksum)

## Security

- Forward secrecy via ephemeral ECDH per session
- Unique per-session encryption key derived via HKDF-SHA256
- Per-chunk AEAD nonces (4B prefix + 8B counter, never reused)
- Encrypted payload authenticated with Poly1305 tag
