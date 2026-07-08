# p2ptransfer — Agent Reference

## Build & Test
```bash
cargo build -p p2ptransfer-core              # Core library
cargo build -p p2ptransfer-cli                # CLI binary (output: target/debug/p2p.exe)
cargo build -p p2ptransfer-gui                # Tauri desktop GUI
cargo test -p p2ptransfer-core -p p2ptransfer-cli  # 89 tests
cargo build --release                         # Release binary (10.8 MB)
```

## Project Structure
```
p2ptransfer-core/         # Core: P2P, crypto, compression, transfer engine
├── src/
│   ├── compress/         # Zstd + LZ4 compression + format detector
│   ├── crypto/           # ECDH (x25519-dalek) + ChaCha20-Poly1305 AEAD
│   ├── network/          # TCP, STUN (RFC 8489), relay, NAT hole punch
│   ├── p2p/              # Discovery (UDP multicast), peer management
│   ├── storage/          # File iteration
│   └── transfer/         # Engine, chunker, hasher (BLAKE3), resume (SQLite)
desktop/
├── p2ptransfer-cli/      # CLI (clap + indicatif + TUI progress)
└── p2ptransfer-gui/      # Tauri v1 + React frontend
mobile/
├── p2ptransfer-ffi/      # cdylib for Flutter (raw FFI)
└── p2ptransfer-flutter/  # Flutter app scaffold (flutter_rust_bridge)
.github/workflows/ci.yml  # CI: check + build-release (5 targets) + packaging
wix/                      # WiX MSI installer source
scripts/                  # macOS .dmg + Linux .deb packaging
```

## Key Dependencies
- **ECDH**: x25519-dalek (Curve25519)
- **AEAD**: chacha20poly1305 (ChaCha20-Poly1305)
- **KDF**: hkdf (HKDF-SHA256)
- **Hash**: blake3 (streaming for large files)
- **Compression**: zstd (adaptive, skip compressed formats)
- **DB**: rusqlite (bundled SQLite for resume tracking)
- **CLI**: clap + indicatif
- **GUI**: tauri v1 + React

## Protocol
- **Wire tags**: CLIENT_HELLO(0x05) → SERVER_HELLO(0x06) → METADATA(0x00) → CHUNK(0x01) → CHUNK_ACK(0x02) → COMPLETE(0x03) | ERROR(0x04)
- **Encryption**: Ephemeral ECDH per session → HKDF → ChaCha20-Poly1305 per chunk
- **Nonce**: 4-byte sender-generated prefix + 8-byte chunk counter

## CLI Commands
- `p2p send <path> <addr>` — Send with ECDH handshake + encrypted chunks + retry
- `p2p listen` — Receive with ECDH handshake + decrypt + verify
- `p2p list` — UDP multicast LAN discovery
- `p2p config --show/--set K=V/--reset` — Config management
- `p2p status` — Show config + active transfers from resume DB
- `p2p --daemon` — Headless background mode

## Phases 1–5 Status
- **Phase 1** ✅ Core library (transfer engine, compression, hashing)
- **Phase 2** ✅ Crypto (ECDH+AEAD), resume, backpressure, CI/CD, signal handling, retry
- **Phase 3** ✅ CLI polish, Tauri GUI (React frontend), WiX MSI, packaging scripts, CI release
- **Phase 4** ✅ Flutter scaffold (pubspec, pages, Rust bridge crate)
- **Phase 5** ✅ STUN client, TCP relay, NAT hole punching

## Key Constraints
- Java 21+ / Gradle references are STALE — this is a pure Rust project
- Binary: `p2p` (cli), `p2ptransfer-gui` (desktop)
- MSYS2 mingw64 path: `C:\msys64\msys64\mingw64\bin` must be first in PATH
- `crate-type = ["lib", "cdylib"]` in p2ptransfer-core
