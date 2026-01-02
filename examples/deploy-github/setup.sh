#!/bin/bash
# Download WIT dependencies from mik-sdk releases
# Usage: ./setup.sh [version]

set -e

VERSION="${1:-latest}"
REPO="dufeut/mik-sdk"

echo "Fetching WIT deps from $REPO ($VERSION)..."

# Create wit/deps directory
mkdir -p wit/deps

# Download and extract wit-deps.tar.gz from release
if [ "$VERSION" = "latest" ]; then
  URL="https://github.com/$REPO/releases/latest/download/wit-deps.tar.gz"
else
  URL="https://github.com/$REPO/releases/download/v$VERSION/wit-deps.tar.gz"
fi

curl -sL "$URL" | tar -xz -C wit/deps

echo "Done. wit/deps ready."
ls wit/deps/
