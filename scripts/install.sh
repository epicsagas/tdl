#!/bin/bash
# tdl installer script
# Usage: curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/epicsagas/tdl/main/scripts/install.sh | sh

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Detect platform
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Linux*)     PLATFORM=linux;;
  Darwin*)    PLATFORM=macos;;
  *)          echo -e "${RED}Unsupported OS: $OS${NC}"; exit 1;;
esac

case "$ARCH" in
  x86_64|amd64)  ARCH_ALT=x86_64;;
  aarch64|arm64) ARCH_ALT=aarch64;;
  arm*)          ARCH_ALT=aarch64;;
  *)             echo -e "${RED}Unsupported architecture: $ARCH${NC}"; exit 1;;
esac

# Get latest release version
LATEST_URL="https://api.github.com/repos/epicsagas/tdl/releases/latest"
VERSION=$(curl -s "$LATEST_URL" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')

if [ -z "$VERSION" ]; then
  echo -e "${YELLOW}Failed to fetch latest version, using default${NC}"
  VERSION="main"
fi

echo -e "${GREEN}Installing tdl $VERSION for $PLATFORM-$ARCH_ALT${NC}"

# Download URL
BINARY_NAME="tdl-${ARCH_ALT}-${PLATFORM}"
DOWNLOAD_URL="https://github.com/epicsagas/tdl/releases/download/${VERSION}/${BINARY_NAME}"

# Install directory
INSTALL_DIR="$HOME/.local/bin"
mkdir -p "$INSTALL_DIR"

echo "Downloading from $DOWNLOAD_URL"
curl --proto '=https' --tlsv1.2 -sSfL "$DOWNLOAD_URL" -o "$INSTALL_DIR/tdl" || {
  echo -e "${RED}Download failed${NC}"
  echo "Try downloading manually from: https://github.com/epicsagas/tdl/releases"
  exit 1
}

chmod +x "$INSTALL_DIR/tdl"

echo -e "${GREEN}✓ Installed to $INSTALL_DIR/tdl${NC}"

# Check PATH
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
  echo -e "${YELLOW}⚠ $INSTALL_DIR is not in your PATH${NC}"
  echo "Add this to your shell profile (~/.bashrc, ~/.zshrc, etc.):"
  echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
fi

echo ""
echo "Run: tdl --version"
