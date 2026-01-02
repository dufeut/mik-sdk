# Deploy to GitHub Container Registry

Complete example for deploying mik-sdk handlers to ghcr.io using pure cargo/wasm tooling.

## Files

```
deploy-github/
├── .github/workflows/
│   └── deploy.yml   # GitHub Actions workflow (ready to use)
├── src/lib.rs       # Example CRUD handler
├── Cargo.toml       # Dependencies (mik-sdk from crates.io)
├── wit/world.wit    # WIT world definition
├── setup.sh         # Local dev setup script
└── README.md
```

## Quick Start

```bash
# 1. Copy this example to a new repo
cp -r examples/deploy-github my-api
cd my-api
git init

# 2. Setup WIT deps for local dev
./setup.sh

# 3. Build and compose locally
./build.sh

# 4. Run locally
wasmtime serve -S cli=y service.wasm

# 5. Push to GitHub - workflow runs automatically
git add .
git commit -m "Initial commit"
git remote add origin git@github.com:your-org/my-api.git
git push -u origin main
```

## What the Workflow Does

1. Fetches WIT deps from mik-sdk releases
2. Builds handler with `cargo component`
3. Pulls bridge from `ghcr.io/dufeut/mik-sdk-bridge`
4. Composes with `wac`
5. Strips debug info with `wasm-tools`
6. Generates OpenAPI schema
7. Pushes single bundle to `ghcr.io/{owner}/{repo}:{version}`

## Output

Single OCI artifact containing both:
```
ghcr.io/your-org/my-api:0.1.0
├── service.wasm      # Composed WASI HTTP component
└── openapi.json      # OpenAPI 3.1 schema
```

## Pull Your Component

```bash
# With oras
oras pull ghcr.io/your-org/my-api:0.1.0

# With mik (compatible)
mik add your-org/my-api
```

## Customization

| Variable         | Default    | Description                          |
| ---------------- | ---------- | ------------------------------------ |
| `SDK_VERSION`    | `0.1.2`    | mik-sdk version (bridge + WIT deps)  |
| `inputs.version` | Cargo.toml | Override version for manual dispatch |

## Local Development

```bash
# First time setup
./setup.sh            # Fetch WIT deps

# Build complete service
./build.sh            # Build, compose, strip

# Run locally
wasmtime serve -S cli=y service.wasm
curl http://localhost:8080/
```

## Prerequisites

- Rust 1.89+

Tools (`cargo-component`, `wac`, `wasm-tools`) are auto-installed by `build.sh` if missing.

## No mik CLI Required

This workflow uses only standard tools:
- `cargo-component` - Build WASM components
- `wac` - Compose components
- `wasm-tools` - Strip debug info
- `oras` - Push to OCI registry
- `curl` - Fetch deps from GitHub releases
