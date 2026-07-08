#!/usr/bin/env bash
set -euo pipefail
# Package p2ptransfer CLI as a .deb package
# Prerequisites: dpkg-deb

SRC_DIR="$(cd "$(dirname "$0")/.." && pwd)"
BUILD_DIR="$SRC_DIR/target/release"
APP_NAME="p2ptransfer"
VERSION="0.1.0"

echo "Building release binary..."
cargo build --release -p p2ptransfer-cli

echo "Creating .deb package..."
DEB_DIR="$BUILD_DIR/deb/$APP_NAME-$VERSION"
mkdir -p "$DEB_DIR/DEBIAN"
mkdir -p "$DEB_DIR/usr/bin"
mkdir -p "$DEB_DIR/usr/share/doc/$APP_NAME"

cp "$BUILD_DIR/p2ptransfer" "$DEB_DIR/usr/bin/$APP_NAME"
chmod 755 "$DEB_DIR/usr/bin/$APP_NAME"

cat > "$DEB_DIR/DEBIAN/control" <<EOF
Package: $APP_NAME
Version: $VERSION
Section: net
Priority: optional
Architecture: amd64
Maintainer: p2ptransfer
Description: Peer-to-peer file transfer tool with ECDH encryption
 Secure, direct P2P file transfers with resume support,
 adaptive compression, and LAN discovery.
EOF

cat > "$DEB_DIR/usr/share/doc/$APP_NAME/changelog" <<EOF
$APP_NAME ($VERSION) stable; urgency=medium

  * Initial release
 -- p2ptransfer <dev@p2ptransfer.app>  $(date -R)
EOF

gzip -9 -n "$DEB_DIR/usr/share/doc/$APP_NAME/changelog"

dpkg-deb --build "$DEB_DIR" "$BUILD_DIR/${APP_NAME}_${VERSION}_amd64.deb"
echo "DEB created: $BUILD_DIR/${APP_NAME}_${VERSION}_amd64.deb"
rm -rf "$BUILD_DIR/deb"
