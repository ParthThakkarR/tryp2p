# WAN Architecture Replacement

## What is being deleted
- `p2ptransfer-core/src/network/nat.rs`: All NAT classification, STUN UDP/TCP hole punching, IPv6 branching, UPnP/NAT-PMP scaffolding.
- `p2ptransfer-core/src/network/stun.rs`: The custom STUN client implementation and binding logic.
- `p2ptransfer-core/src/network/relay.rs`: The self-hosted TCP relay server, directory service, HMAC tokens, and SQLite directory DB.
- Any CLI flags and configuration blocks related to relay server address, NAT detection, STUN servers, and UDP ports in `p2ptransfer-cli/src/main.rs`.
- The stale Java/Gradle tree (`p2p-*` directories, gradle scripts, Docker files) left over from the previous implementation.

## What is being added
- **copyparty integration**: Code to auto-download and run `copyparty` as a local HTTP file server in a short-lived download-only mode against a single file/temp-directory.
- **cloudflared integration**: Code to auto-download and run `cloudflared tunnel` (Quick Tunnels) to expose the local `copyparty` instance over an outbound-only connection to Cloudflare's edge, entirely bypassing NAT issues.
- **Manifests & URL routing**: A mechanism for the sender to bundle the `cloudflared` public URL, random path token, and BLAKE3 checksum into a single link.
- **HTTPS Downloader**: Logic in `p2p receive <link>` to parse the manifest, fetch the file via HTTP `Range` requests over HTTPS, verify checksum, and show a progress bar.

## Distribution Decisions (Copyparty & Cloudflared)
Based on official GitHub releases for each tool:

### copyparty
- **Windows**: We will auto-download the native `copyparty.exe` executable.
- **macOS/Linux**: Since no native standalone binaries are provided for these platforms, we will fallback to downloading `copyparty-sfx.py`. This is a degraded path requiring Python 3 (`python3`) to be installed on the host. Before spawning on macOS/Linux, we will check for `python3` on `$PATH` and fail immediately with a clear actionable error if absent (checked as part of Phase 3).

### cloudflared
- **Windows**: We will auto-download `cloudflared-windows-amd64.exe`.
- **macOS**: We will auto-download `cloudflared-darwin-amd64.tgz` (or arm64), extract the binary, and explicitly strip the quarantine attribute (`xattr -d com.apple.quarantine <path>`) before spawning.
- **Linux**: We will auto-download `cloudflared-linux-amd64` (or arm64) native binaries, and explicitly `chmod +x` them before spawning.

Both tools will be downloaded on first-use, cached in the application's config/data directory, and checksum verified against hardcoded or fetched signatures.

## Version Pinning & Checksum Verification
- We pin exact versions of our dependencies in a single constants file: `p2ptransfer-core/src/network/wan_constants.rs`.
- **Current Pins**: 
  - `copyparty`: **v1.20.17**
  - `cloudflared`: **2026.7.0**
- We will fetch the official published checksums for these exact releases and hardcode them in `wan_constants.rs`. Any checksum mismatch during download will fail loudly and refuse to run; there is no silent fallback.
- **Bump Procedure**: To update a dependency, edit `wan_constants.rs` to change both the version string and the official checksum simultaneously. Do not touch anything else.

## Lifecycle & Transfer Completion
- **No receiver beacon**: The receiver will not hit any write/beacon endpoint. Copyparty remains strictly read-only/download-only.
- **Primary Mechanism (Idle Timeout)**: The sender CLI will parse its copyparty subprocess stdout/access-log to track the last successfully served byte range. If there is no activity for `N` minutes, copyparty and cloudflared are torn down automatically.
- **Secondary Mechanism (Manual Backstop)**: The sender CLI will display a "Press Enter once transfer is confirmed complete" prompt, allowing the user to tear down the tunnel explicitly at any time, overriding the timeout.

## Manifest Transport
- The manifest (filename, size, BLAKE3 checksum) is NOT encoded in the shareable link.
- Instead, the sender writes a small `manifest.json` into the single-use staging directory.
- The receiver fetches the manifest via HTTPS from the same random token path (e.g., `https://xxxxx.trycloudflare.com/<random_token>/manifest.json`) before downloading the actual file.

## Third-Party Licenses
- `copyparty` is licensed under MIT.
- `cloudflared` is licensed under Apache-2.0.
- A `THIRD_PARTY_LICENSES` file covering both will be generated and bundled with all redistributable binaries during the Phase 9 packaging step.
