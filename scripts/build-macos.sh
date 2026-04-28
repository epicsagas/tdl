#!/usr/bin/env bash
# Build macOS bundles (universal: arm64 + x86_64) and collect into dist/
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

VER=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
echo "==> Building tdl v${VER} for macOS (universal)"

# Ensure both targets are installed
rustup target add aarch64-apple-darwin x86_64-apple-darwin

# Install tauri-cli if needed
if ! cargo tauri --version &>/dev/null 2>&1; then
    cargo install tauri-cli --version "^2" --locked
fi

# Build universal binary
cargo tauri build \
    --features gui \
    --target universal-apple-darwin \
    --bundles app,dmg

BUNDLE_DIR="target/universal-apple-darwin/release/bundle"
mkdir -p dist

# .app → tar.gz (arm64 label — universal binary runs on both)
APP=$(find "$BUNDLE_DIR/macos" -maxdepth 1 -name "*.app" -type d | head -1)
if [ -n "$APP" ]; then
    OUT="dist/tdl_${VER}_darwin_universal.app.tar.gz"
    tar -czf "$OUT" -C "$(dirname "$APP")" "$(basename "$APP")"
    echo "  app  → $OUT"
fi

# .dmg
DMG=$(find "$BUNDLE_DIR/dmg" -maxdepth 1 -name "*.dmg" | head -1)
if [ -n "$DMG" ]; then
    OUT="dist/tdl_${VER}_darwin_universal.dmg"
    cp "$DMG" "$OUT"
    echo "  dmg  → $OUT"
fi

# CLI binary (standalone, no GUI)
BIN="target/universal-apple-darwin/release/tdl"
if [ -f "$BIN" ]; then
    OUT="dist/tdl_${VER}_darwin_universal.tar.gz"
    tar -czf "$OUT" -C "$(dirname "$BIN")" tdl
    echo "  cli  → $OUT"
fi

echo "==> macOS build complete: $(ls dist/tdl_${VER}_darwin_* | wc -l | tr -d ' ') files"
