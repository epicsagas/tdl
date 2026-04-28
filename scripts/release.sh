#!/usr/bin/env bash
# Full local release: build macOS + Linux, upload to GitHub Release.
# Windows is built separately via GitHub Actions (release-windows.yml).
#
# Usage:
#   ./scripts/release.sh              # build all, upload to current tag
#   ./scripts/release.sh --skip-build # upload existing dist/ only
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

SKIP_BUILD=0
for arg in "$@"; do
    [[ "$arg" == "--skip-build" ]] && SKIP_BUILD=1
done

VER=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
TAG="tdl-v${VER}"

# Require gh cli
if ! command -v gh &>/dev/null; then
    echo "error: gh CLI not found. Install via: brew install gh" && exit 1
fi

if [ "$SKIP_BUILD" -eq 0 ]; then
    echo "==> Step 1/3: macOS build"
    bash "$ROOT/scripts/build-macos.sh"

    echo "==> Step 2/3: Linux amd64 build (Podman)"
    bash "$ROOT/scripts/build-linux.sh" amd64

    echo "==> Step 3/3: Linux arm64 build (Podman + QEMU)"
    bash "$ROOT/scripts/build-linux.sh" arm64
else
    echo "==> Skipping build, uploading existing dist/ files"
fi

# Collect all dist files for this version
FILES=()
while IFS= read -r f; do
    FILES+=("$f")
done < <(find dist -name "tdl_${VER}_*" -type f | sort)

if [ "${#FILES[@]}" -eq 0 ]; then
    echo "error: no files found in dist/ for v${VER}" && exit 1
fi

echo "==> Files to upload (${#FILES[@]}):"
for f in "${FILES[@]}"; do
    echo "    $f  ($(du -sh "$f" | cut -f1))"
done

# Generate checksums
CHECKSUM_FILE="dist/SHA256SUMS.txt"
(cd dist && shasum -a 256 $(ls tdl_${VER}_* 2>/dev/null) > "$(basename "$CHECKSUM_FILE")")
FILES+=("$CHECKSUM_FILE")
echo "    $CHECKSUM_FILE"

# Create or update GitHub Release
PUBLIC_REPO="${PUBLIC_RELEASE_REPO:-}"
if [ -n "$PUBLIC_REPO" ]; then
    REPO_FLAG="--repo $PUBLIC_REPO"
    echo "==> Publishing to public repo: $PUBLIC_REPO"
else
    REPO_FLAG=""
    echo "==> Publishing to current repo"
fi

if gh release view "$TAG" $REPO_FLAG &>/dev/null 2>&1; then
    echo "==> Release $TAG already exists, uploading files..."
else
    echo "==> Creating release $TAG..."
    gh release create "$TAG" $REPO_FLAG \
        --title "$TAG" \
        --notes "See CHANGELOG for details."
fi

gh release upload "$TAG" "${FILES[@]}" $REPO_FLAG --clobber

echo "==> Release $TAG published successfully"
echo "    https://github.com/${PUBLIC_REPO:-$(gh repo view --json nameWithOwner -q .nameWithOwner)}/releases/tag/${TAG}"
