#!/usr/bin/env bash
# Build Linux GUI bundles via Podman container.
# Usage: ./scripts/build-linux.sh [amd64|arm64]  (default: amd64)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

ARCH="${1:-amd64}"
VER=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')

case "$ARCH" in
    amd64) RUST_TARGET="x86_64-unknown-linux-gnu"  ; PLATFORM="linux/amd64" ;;
    arm64) RUST_TARGET="aarch64-unknown-linux-gnu" ; PLATFORM="linux/arm64" ;;
    *) echo "Usage: $0 [amd64|arm64]" && exit 1 ;;
esac

echo "==> Building tdl v${VER} for Linux ${ARCH} via Podman"

IMAGE="tdl-builder-linux-${ARCH}"

# Build container image if not cached
if ! podman image exists "$IMAGE"; then
    echo "  Building container image ${IMAGE}..."
    podman build \
        --platform "$PLATFORM" \
        -t "$IMAGE" \
        -f - "$ROOT" <<'DOCKERFILE'
FROM ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y \
    curl git pkg-config \
    libwebkit2gtk-4.1-dev \
    libgtk-3-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    patchelf \
    libssl-dev \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
ENV PATH="/root/.cargo/bin:$PATH"
RUN cargo install tauri-cli --version "^2" --locked
DOCKERFILE
fi

mkdir -p dist

# Run build inside container
podman run --rm \
    --platform "$PLATFORM" \
    -v "$ROOT:/app:z" \
    -w /app \
    "$IMAGE" \
    bash -c "
        set -euo pipefail
        export PATH=\"/root/.cargo/bin:\$PATH\"
        rustup target add ${RUST_TARGET}
        cargo tauri build --features gui --target ${RUST_TARGET} --bundles deb,appimage
    "

BUNDLE_DIR="target/${RUST_TARGET}/release/bundle"

# .deb
DEB=$(find "$BUNDLE_DIR/deb" -name "*.deb" 2>/dev/null | head -1)
if [ -n "$DEB" ]; then
    OUT="dist/tdl_${VER}_linux_${ARCH}.deb"
    cp "$DEB" "$OUT"
    echo "  deb      → $OUT"
fi

# .AppImage
APPIMG=$(find "$BUNDLE_DIR/appimage" -name "*.AppImage" 2>/dev/null | head -1)
if [ -n "$APPIMG" ]; then
    OUT="dist/tdl_${VER}_linux_${ARCH}.AppImage"
    cp "$APPIMG" "$OUT"
    echo "  AppImage → $OUT"
fi

# CLI binary
BIN="target/${RUST_TARGET}/release/tdl"
if [ -f "$BIN" ]; then
    OUT="dist/tdl_${VER}_linux_${ARCH}.tar.gz"
    tar -czf "$OUT" -C "$(dirname "$BIN")" tdl
    echo "  cli      → $OUT"
fi

echo "==> Linux ${ARCH} build complete"
