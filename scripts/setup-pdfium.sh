#!/usr/bin/env bash
# Download the pdfium dynamic library used at runtime by render_page_to_png().
# Mirrors the "Download pdfium" step in .github/workflows/build.yml so local
# dev and CI use the same upstream artifact.
#
# Usage: bash scripts/setup-pdfium.sh        (or `npm run setup:pdfium`)
#
# To bump the upstream version, change PDFIUM_TAG below; the same tag is used
# by CI in .github/workflows/build.yml — keep them in sync.

set -euo pipefail

PDFIUM_TAG="chromium/7665"
DEST="src-tauri/libs/pdfium/lib"

case "$(uname -s)-$(uname -m)" in
  Darwin-arm64)
    asset="pdfium-mac-arm64.tgz"
    lib_in_archive="lib/libpdfium.dylib"
    ;;
  Darwin-x86_64)
    asset="pdfium-mac-x64.tgz"
    lib_in_archive="lib/libpdfium.dylib"
    ;;
  Linux-x86_64)
    asset="pdfium-linux-x64.tgz"
    lib_in_archive="lib/libpdfium.so"
    ;;
  Linux-aarch64 | Linux-arm64)
    asset="pdfium-linux-arm64.tgz"
    lib_in_archive="lib/libpdfium.so"
    ;;
  MINGW*-x86_64 | MSYS*-x86_64 | CYGWIN*-x86_64)
    asset="pdfium-win-x64.tgz"
    lib_in_archive="bin/pdfium.dll"
    ;;
  *)
    echo "unsupported platform: $(uname -s)-$(uname -m)" >&2
    echo "edit scripts/setup-pdfium.sh to add it." >&2
    exit 1
    ;;
esac

# URL-encode the slash in the tag for GitHub releases.
encoded_tag="${PDFIUM_TAG//\//%2F}"
url="https://github.com/bblanchon/pdfium-binaries/releases/download/${encoded_tag}/${asset}"

mkdir -p "$DEST"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

echo "downloading $asset (tag $PDFIUM_TAG)..."
curl -fL "$url" -o "$tmp/pdfium.tgz"
tar xzf "$tmp/pdfium.tgz" -C "$tmp"

if [ ! -f "$tmp/$lib_in_archive" ]; then
  echo "expected $lib_in_archive in archive but did not find it" >&2
  exit 1
fi

cp "$tmp/$lib_in_archive" "$DEST/"
echo "✓ installed $(basename "$lib_in_archive") to $DEST/"
