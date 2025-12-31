# Contributing to mik-sdk

Thank you for your interest in contributing to mik-sdk!

> **Note:** This is version 0.1.0 (early development). The API may change between minor versions.

## Code of Conduct

Be respectful and constructive in all interactions. We welcome contributors of all experience levels.

## Getting Started

1. Fork the repository
2. Clone your fork locally
3. Set up the development environment (see below)
4. Create a feature branch from `main`
5. Make your changes
6. Run all quality checks
7. Submit a pull request

## Development Setup

### Prerequisites

- Rust 1.89+ (Edition 2024)
- Git

### Optional (for WASM development)

- `cargo-component` - Build WASM components
- `wac` - Compose components
- `wasmtime` - Run WASM components

### Building

```bash
# Build all crates
cargo build --all

# Build with all features
cargo build --all-features

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

## Quality Requirements

All contributions must pass these quality gates before merging:

### 1. Code Formatting

Code must be formatted with `cargo fmt`:

```bash
cargo fmt --all
```

Check formatting without modifying files:

```bash
cargo fmt --all --check
```

### 2. Clippy Lints

All clippy warnings must be addressed:

```bash
# Check with all features
cargo clippy --workspace --exclude hello-world --exclude crud-api --exclude auth-api --exclude external-api --all-features -- -D warnings

# Check without default features
cargo clippy -p mik-sdk --no-default-features -- -D warnings
```

### 3. Tests

All tests must pass:

```bash
# Run all tests
cargo test --workspace --exclude hello-world --exclude crud-api --exclude auth-api --exclude external-api

# Run with all features
cargo test --all-features --workspace --exclude hello-world --exclude crud-api --exclude auth-api --exclude external-api

# Run doc tests
cargo test --doc --workspace --exclude hello-world --exclude crud-api --exclude auth-api --exclude external-api
```

### 4. Documentation

Documentation must build without warnings:

```bash
RUSTDOCFLAGS="-Dwarnings" cargo doc --no-deps --all-features --workspace --exclude hello-world --exclude crud-api --exclude auth-api --exclude external-api
```

### 5. Semantic Versioning

For changes to published crates, run semver checks:

```bash
cargo semver-checks check-release -p mik-sdk
cargo semver-checks check-release -p mik-sql
```

See [`.notes/semver-checks.md`](.notes/semver-checks.md) for installation and usage details.

### Quick Check Script

Run all checks before submitting:

```bash
# Format
cargo fmt --all

# Lint
cargo clippy --workspace --exclude hello-world --exclude crud-api --exclude auth-api --exclude external-api --all-features -- -D warnings

# Test
cargo test --workspace --exclude hello-world --exclude crud-api --exclude auth-api --exclude external-api --all-features

# Docs
RUSTDOCFLAGS="-Dwarnings" cargo doc --no-deps --all-features
```

## Code Style

- Follow Rust 2024 edition idioms
- Use `cargo fmt` before committing
- Run `cargo clippy --all` and address warnings
- Add tests for new functionality
- Keep commits atomic and descriptive

### Documentation Standards

All public items should have:

1. A summary line explaining what it does
2. `# Examples` section with runnable code
3. `# Errors` section for fallible functions
4. `# Panics` section if the function can panic

Example:
```rust
/// Parses a JSON string into a value.
///
/// # Examples
///
/// ```
/// use mik_sdk::json;
///
/// let value = json::try_parse(b"{\"name\": \"Alice\"}").unwrap();
/// assert_eq!(value.path_str(&["name"]), Some("Alice".to_string()));
/// ```
///
/// # Errors
///
/// Returns `ParseError::InvalidFormat` if the input is not valid JSON.
pub fn try_parse(input: &[u8]) -> Result<JsonValue, ParseError> {
    // ...
}
```

### API Design Guidelines

- Use `#[must_use]` for functions whose return value shouldn't be ignored
- Use `#[non_exhaustive]` on enums and structs that may grow
- Implement standard traits: `Debug`, `Clone`, `PartialEq`, `Eq`, `Default` where appropriate
- Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)

## Commit Messages

Use conventional commits:

```
feat(sdk): add new response macro
fix(sql): correct cursor pagination for DESC sort
docs: update README examples
refactor(json): simplify parsing logic
chore: update dependencies
```

| Prefix | Description |
|--------|-------------|
| `feat` | New feature |
| `fix` | Bug fix |
| `docs` | Documentation only |
| `style` | Code style (formatting, etc.) |
| `refactor` | Code change that neither fixes a bug nor adds a feature |
| `perf` | Performance improvement |
| `test` | Adding or updating tests |
| `chore` | Maintenance tasks |

## Pull Request Process

### Before Submitting

1. Ensure all quality checks pass locally
2. Update documentation if adding/changing public API
3. Add tests for new functionality
4. Update CHANGELOG.md for significant changes

### PR Requirements

- **Title**: Use conventional commit format (e.g., `feat(sdk): add new feature`)
- **Description**: Explain what and why, not just how
- **Tests**: Include tests for new functionality
- **Documentation**: Update docs for public API changes

### Review Process

1. CI must pass (format, clippy, tests, docs)
2. SemVer check must pass for library changes (on PRs)
3. At least one maintainer approval required
4. Squash and merge preferred for clean history

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

## Breaking Changes

Breaking changes require:

1. Major version bump (or minor for pre-1.0)
2. Clear documentation in CHANGELOG.md
3. Migration guide if significant
4. Deprecation period if possible

Use `#[deprecated]` to warn users before removal:

```rust
#[deprecated(since = "0.2.0", note = "Use `new_function` instead")]
pub fn old_function() { ... }
```

## License

By contributing to mik-sdk, you agree that your contributions will be licensed under the same license as the project (MIT OR Apache-2.0).

## Questions?

Open an issue for questions or discussion.
