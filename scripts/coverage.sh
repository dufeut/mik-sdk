#!/bin/bash
# Coverage script for mik-sdk (CI/Linux/macOS)
# Requires: cargo install cargo-llvm-cov

set -e

echo "=== mik-sdk Coverage ==="

# Check if cargo-llvm-cov is installed
if ! command -v cargo-llvm-cov &> /dev/null; then
    echo "Error: cargo-llvm-cov is not installed."
    echo "Install with: cargo install cargo-llvm-cov"
    exit 1
fi

# Run tests with coverage and generate LCOV
echo ""
echo "Generating coverage data..."
cargo llvm-cov --all-features --workspace --lcov --output-path target/lcov.info

# Show summary and enforce minimum coverage
echo ""
echo "=== Coverage Summary ==="
cargo llvm-cov report --all-features --workspace --fail-under-lines 80

echo ""
echo "=== Coverage Complete ==="
echo "LCOV file: target/lcov.info"
