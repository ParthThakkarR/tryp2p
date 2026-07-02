#!/usr/bin/env bash
# Cross-platform build script for P2P File Transfer
# Detects OS and builds all available package formats
# Usage: ./build-package.sh [--test | --skip-tests] [--target all|app|dmg|deb|jar]
set -euo pipefail

GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${CYAN}============================================${NC}"
echo -e "${CYAN}  P2P File Transfer - Cross-Platform Build${NC}"
echo -e "${CYAN}============================================${NC}"
echo ""

OS="$(uname -s)"
ARCH="$(uname -m)"
echo -e "  OS:   ${GREEN}${OS}${NC}"
echo -e "  Arch: ${GREEN}${ARCH}${NC}"
echo ""

# Parse args
RUN_TESTS=true
TARGET="all"
for arg in "$@"; do
  case "$arg" in
    --skip-tests) RUN_TESTS=false ;;
    --test) RUN_TESTS=true ;;
    --target=*) TARGET="${arg#*=}" ;;
    *) echo -e "${RED}Unknown arg: $arg${NC}"; exit 1 ;;
  esac
done

# --- Step 1: Tests ---
if [ "$RUN_TESTS" = true ]; then
  echo -e "${YELLOW}[1/3] Running tests...${NC}"
  ./gradlew test 2>&1
  echo -e "  ${GREEN}All tests pass.${NC}"
  echo ""
else
  echo -e "${YELLOW}[1/3] Skipping tests.${NC}"
  echo ""
fi

# --- Step 2: Shadow JAR ---
echo -e "${YELLOW}[2/3] Building universal shadow JAR...${NC}"
./gradlew :p2p-app:shadowJar 2>&1
JAR_PATH="p2p-app/build/libs/p2p-1.0.0-SNAPSHOT.jar"
JAR_SIZE=$(du -h "$JAR_PATH" 2>/dev/null | cut -f1 || echo "?")
echo -e "  ${GREEN}Shadow JAR: $JAR_PATH ($JAR_SIZE)${NC}"
echo ""

# --- Step 3: Platform-specific builds ---
echo -e "${YELLOW}[3/3] Building platform packages...${NC}"
echo ""

BUILT=()

case "$OS" in
  Darwin*)
    echo -e "  macOS detected — building .app and .dmg${NC}"

    # .app bundle
    if [ "$TARGET" = "all" ] || [ "$TARGET" = "app" ]; then
      echo -e "  ${YELLOW}>> .app bundle...${NC}"
      if ./gradlew :p2p-app:packageMacApp 2>&1; then
        APP_PATH="p2p-app/build/dist-mac-app/P2PTransfer.app"
        if [ -d "$APP_PATH" ]; then
          SIZE=$(du -sh "$APP_PATH" | cut -f1)
          echo -e "    ${GREEN}$APP_PATH ($SIZE)${NC}"
          BUILT+=("macOS .app: $APP_PATH")
        fi
      else
        echo -e "    ${RED}FAILED${NC}"
      fi
    fi

    # .dmg installer
    if [ "$TARGET" = "all" ] || [ "$TARGET" = "dmg" ]; then
      echo -e "  ${YELLOW}>> .dmg installer...${NC}"
      if ./gradlew :p2p-app:packageMacDmg 2>&1; then
        DMG_PATH=$(find p2p-app/build/dist-mac-dmg -name "*.dmg" 2>/dev/null | head -1)
        if [ -n "$DMG_PATH" ]; then
          SIZE=$(du -h "$DMG_PATH" | cut -f1)
          echo -e "    ${GREEN}$DMG_PATH ($SIZE)${NC}"
          BUILT+=("macOS DMG: $DMG_PATH")
        fi
      else
        echo -e "    ${RED}FAILED${NC}"
      fi
    fi
    ;;

  Linux*)
    echo -e "  Linux detected — building app image and .deb${NC}"

    # App image
    if [ "$TARGET" = "all" ] || [ "$TARGET" = "app" ]; then
      echo -e "  ${YELLOW}>> App image...${NC}"
      if ./gradlew :p2p-app:packageLinuxApp 2>&1; then
        APP_PATH=$(find p2p-app/build/dist-linux-app -type d -name "P2PTransfer" 2>/dev/null | head -1)
        if [ -n "$APP_PATH" ]; then
          SIZE=$(du -sh "$APP_PATH" | cut -f1)
          echo -e "    ${GREEN}$APP_PATH ($SIZE)${NC}"
          BUILT+=("Linux app image: $APP_PATH")
        fi
      else
        echo -e "    ${RED}FAILED${NC}"
      fi
    fi

    # .deb package
    if [ "$TARGET" = "all" ] || [ "$TARGET" = "deb" ]; then
      echo -e "  ${YELLOW}>> .deb package...${NC}"
      if dpkg -l | grep -q fakeroot 2>/dev/null; then
        if ./gradlew :p2p-app:packageLinuxDeb 2>&1; then
          DEB_PATH=$(find p2p-app/build/dist-linux-deb -name "*.deb" 2>/dev/null | head -1)
          if [ -n "$DEB_PATH" ]; then
            SIZE=$(du -h "$DEB_PATH" | cut -f1)
            echo -e "    ${GREEN}$DEB_PATH ($SIZE)${NC}"
            BUILT+=("Linux DEB: $DEB_PATH")
          fi
        else
          echo -e "    ${RED}FAILED${NC}"
        fi
      else
        echo -e "    ${YELLOW}  fakeroot not found — DEB requires it. Install: sudo apt install fakeroot${NC}"
      fi
    fi
    ;;

  CYGWIN*|MINGW*|MSYS*)
    echo -e "  Windows (Git Bash/MSYS2) detected${NC}"
    echo -e "  ${YELLOW}  Use build-installer.ps1 on Windows PowerShell for best results.${NC}"
    echo -e "  ${YELLOW}  Falling back to JAR only.${NC}"
    BUILT+=("Universal JAR (JDK 21+): $JAR_PATH")
    ;;

  *)
    echo -e "  Unknown OS. Building JAR only.${NC}"
    BUILT+=("Universal JAR (JDK 21+): $JAR_PATH")
    ;;
esac

# JAR is always available
BUILT+=("Universal JAR (JDK 21+): $JAR_PATH ($JAR_SIZE)")

echo ""
echo -e "${CYAN}============================================${NC}"
echo -e "${CYAN}  BUILD COMPLETE${NC}"
echo -e "${CYAN}============================================${NC}"
echo ""
for item in "${BUILT[@]}"; do
  echo -e "  ${GREEN}✓${NC} $item"
done
echo ""

# Print run instructions
echo -e "${YELLOW}Run instructions:${NC}"
echo ""
case "$OS" in
  Darwin*)
    echo "  P2PTransfer.app     Double-click to launch"
    echo "  P2PTransfer.dmg     Double-click to install"
    ;;
  Linux*)
    echo "  P2PTransfer/        Run ./P2PTransfer/bin/P2PTransfer"
    echo "  P2PTransfer.deb     sudo dpkg -i <file>"
    ;;
esac
echo "  java -jar p2p-1.0.0-SNAPSHOT.jar --gui   (JDK 21+ required)"
echo ""
