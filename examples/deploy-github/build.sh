#!/bin/bash
# Build and compose the complete WASI HTTP component
# Usage: ./build.sh [version]
#
# Prerequisites: cargo-component, wac
# Optional: wasm-tools (for stripping debug info)

set -e

VERSION="${1:-0.1.2}"
REPO="dufeut/mik-sdk"

echo "==> Building handler..."
cargo component build --release

# Find the built wasm
HANDLER=$(find target -path "*/release/*.wasm" ! -path "*/deps/*" | head -1)
if [ -z "$HANDLER" ]; then
  echo "ERROR: No handler wasm found"
  exit 1
fi
echo "    Handler: $HANDLER"

# Download bridge if not cached
BRIDGE="target/mik-bridge-${VERSION}.wasm"
if [ ! -f "$BRIDGE" ]; then
  echo "==> Downloading bridge v${VERSION}..."
  curl -sL "https://github.com/$REPO/releases/download/v$VERSION/mik-bridge.wasm" -o "$BRIDGE"
fi
echo "    Bridge: $BRIDGE"

echo "==> Composing components..."
wac plug "$BRIDGE" --plug "$HANDLER" -o service.wasm

# Strip if wasm-tools available
if command -v wasm-tools &> /dev/null; then
  echo "==> Stripping debug info..."
  wasm-tools strip --all service.wasm -o service.wasm
fi

SIZE=$(ls -lh service.wasm | awk '{print $5}')
echo ""
echo "Done: service.wasm ($SIZE)"
echo ""
echo "Run with:"
echo "  wasmtime serve -S cli=y service.wasm"
echo "  spin up -f spin.toml"
