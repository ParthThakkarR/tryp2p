#!/usr/bin/env bash
set -euo pipefail
# Package p2ptransfer CLI as a macOS .app bundle and .dmg
# Prerequisites: create-dmg (brew install create-dmg)

SRC_DIR="$(cd "$(dirname "$0")/.." && pwd)"
BUILD_DIR="$SRC_DIR/target/release"
APP_NAME="p2ptransfer"
VERSION="0.1.0"

echo "Building release binary..."
cargo build --release -p p2ptransfer-cli

echo "Creating .app bundle..."
APP_DIR="$BUILD_DIR/$APP_NAME.app/Contents/MacOS"
mkdir -p "$APP_DIR"
cp "$BUILD_DIR/p2ptransfer" "$APP_DIR/$APP_NAME"

cat > "$BUILD_DIR/$APP_NAME.app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key>
  <string>$APP_NAME</string>
  <key>CFBundleIdentifier</key>
  <string>com.p2ptransfer.cli</string>
  <key>CFBundleName</key>
  <string>$APP_NAME</string>
  <key>CFBundleVersion</key>
  <string>$VERSION</string>
  <key>CFBundleShortVersionString</key>
  <string>$VERSION</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
</dict>
</plist>
EOF

echo "Creating .dmg..."
if command -v create-dmg &>/dev/null; then
  create-dmg \
    --volname "$APP_NAME $VERSION" \
    --window-pos 200 120 \
    --window-size 600 400 \
    --icon-size 100 \
    --app-drop-link 400 120 \
    "$BUILD_DIR/$APP_NAME-$VERSION.dmg" \
    "$BUILD_DIR/$APP_NAME.app"
  echo "DMG created: $BUILD_DIR/$APP_NAME-$VERSION.dmg"
else
  echo "create-dmg not installed. .app bundle at $APP_DIR"
fi
