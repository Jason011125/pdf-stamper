#!/usr/bin/env bash
# Run once after the container is created. Pulls the platform-specific
# pdfium binary, installs node deps, and warms the cargo crate cache.

set -euo pipefail
cd /workspace

echo "→ Installing pdfium (via scripts/setup-pdfium.sh)..."
bash scripts/setup-pdfium.sh

# /workspace/node_modules is a Docker named volume (see devcontainer.json).
# Docker initializes it root:root on first mount; fix ownership so npm
# (running as `node`) can write to it. NOPASSWD chown is granted in the
# Dockerfile sudoers rule.
echo "→ Fixing /workspace/node_modules ownership (Docker volume init)..."
sudo /bin/chown node:node /workspace/node_modules

echo "→ npm ci..."
npm ci

echo "→ cargo fetch (warming crate cache; first run is slow)..."
( cd src-tauri && cargo fetch )

echo
echo "post-create complete."
echo
echo "Verify the three Ralph feedback loops manually:"
echo "  cd src-tauri && cargo test       # Rust + pdfium"
echo "  npx tsc --noEmit                 # TS types"
echo "  npx vitest run                   # frontend unit tests"
