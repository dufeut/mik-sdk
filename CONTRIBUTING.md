# Contributing to mik-sdk

Thank you for your interest in contributing to mik-sdk!

> **Note:** This is version 0.0.1 (experimental). The API is still stabilizing.

## Development Setup

### Prerequisites

- Rust 1.85+ (Edition 2024)
- `cargo-component` for building WASM components
- `wac` for component composition

### Building

```bash
# Build all crates
cargo build --all

# Build in release mode
cargo build --all --release
```

### Testing

```bash
# Run all tests
cargo test --all

# Run tests for specific crate
cargo test -p mik-sdk
cargo test -p mik-sql
cargo test -p mik-sdk-macros

# Update snapshot tests
cargo insta review

# Run benchmarks
cargo bench -p mik-sdk
cargo bench -p mik-sql
```

### Building WASM Components

```bash
# Build bridge component
cd mik-bridge && cargo component build --release

# Build example handler
cd examples/hello-world && cargo component build --release

# Compose components
wac plug mik-bridge.wasm --plug handler.wasm -o service.wasm
```

## Code Style

- Follow Rust 2024 edition idioms
- Use `cargo fmt` before committing
- Run `cargo clippy --all` and address warnings
- Add tests for new functionality
- Keep commits atomic and descriptive

## Commit Messages

Use conventional commits:

```
feat(sdk): add new response macro
fix(sql): correct cursor pagination for DESC sort
docs: update README examples
chore: update dependencies
```

## Pull Request Process

1. Fork the repository
2. Create a feature branch from `main`
3. Make your changes with tests
4. Run `cargo test --all` and `cargo clippy --all`
5. Submit a pull request

## Architecture Notes

### Crate Structure

- `mik-sdk` - Main SDK (HTTP, JSON, typed inputs)
- `mik-sdk-macros` - Procedural macros
- `mik-sql` - SQL query builder (standalone)
- `mik-sql-macros` - SQL macros
- `mik-bridge` - WASI HTTP bridge component

### Key Design Decisions

1. **Two-component architecture** - Handler + Bridge composition
2. **Pure Rust JSON** - No cross-component calls for JSON/time/random
3. **Compile-time SQL** - Macro generates SQL at compile time
4. **RFC 7807 errors** - Standard error format

## Questions?

Open an issue for questions or discussion.
