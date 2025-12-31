#!/bin/sh
# Set up git hooks for this repository

echo "Setting up git hooks..."
git config core.hooksPath .githooks
chmod +x .githooks/*
echo "✅ Git hooks configured!"
echo ""
echo "Pre-commit hook will run:"
echo "  - cargo fmt --check"
echo "  - cargo clippy"
echo "  - cargo check"
echo ""
echo "Only on commits that include Rust files (*.rs, Cargo.toml, Cargo.lock)"
