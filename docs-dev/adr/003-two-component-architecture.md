# ADR-003: Two-Component Architecture

## Status

Accepted

## Context

WASI HTTP defines `wasi:http/incoming-handler` for handling requests. WASI P2 runtimes expect components to export this interface.

The problem: WASI HTTP is **low-level**. Handlers must:
- Read streaming body bytes
- Parse headers from tuples
- Construct outgoing responses with proper resource management
- Handle WASI-specific error types

This is tedious, error-prone, and runtime-coupled.

## Decision

Split into **two components** composed at build time:

```
┌─────────────────────────────────────────────────────────┐
│  Handler Component (your code)                          │
│  - Exports: mik:core/handler                            │
│  - Simple Request/Response types                        │
│  - No WASI HTTP knowledge                               │
└─────────────────────────────────────────────────────────┘
                          ↓ compose (wac plug)
┌─────────────────────────────────────────────────────────┐
│  Bridge Component (mik-bridge)                          │
│  - Exports: wasi:http/incoming-handler                  │
│  - Imports: mik:core/handler                            │
│  - Translates WASI HTTP ↔ mik types                     │
└─────────────────────────────────────────────────────────┘
                          ↓ runs on
┌─────────────────────────────────────────────────────────┐
│  Any WASI HTTP Runtime                                  │
└─────────────────────────────────────────────────────────┘
```

### Composition command

```bash
wac plug mik-bridge.wasm --plug handler.wasm -o service.wasm
```

### Interface (mik:core/handler)

```wit
interface handler {
    record request-data {
        method: method,
        path: string,
        headers: list<tuple<string, string>>,
        body: option<list<u8>>,
    }

    record response {
        status: u16,
        headers: list<tuple<string, string>>,
        body: option<list<u8>>,
    }

    handle: func(req: request-data) -> response;
}
```

Simple, sync, no streaming, no resources. The bridge handles complexity.

## Consequences

### Positive

- **Simple handler code** - No WASI boilerplate, just Request → Response
- **Portable** - Handlers work on any runtime without modification
- **Testable** - Handler logic tests without WASI environment
- **Shared bridge** - 85KB bridge cost amortized across all handlers
- **Runtime evolution** - WASI HTTP changes only affect bridge, not handlers

### Negative

- **Extra build step** - Must compose after building (`wac plug`)
- **No streaming** - Bridge buffers full request/response (acceptable for typical HTTP)
- **Sync only** - No async handler support (WASI async still evolving)

### Neutral

- Composed binary larger than minimal hand-written WASI HTTP handler
- Two WIT files to maintain (mik-wit and bridge bindings)

## Alternatives Considered

### Single component exporting wasi:http directly

Rejected: Every handler would need WASI HTTP boilerplate. Code duplication, harder testing, runtime-coupled.

### Code generation instead of composition

Rejected: Would generate WASI HTTP code into each handler. Larger handlers, harder to update bridge logic.

### Runtime adapter layer

Rejected: Would require runtime-specific builds. Composition gives us runtime portability from a single handler binary.

### Async streaming handlers

Deferred: WASI async is still evolving (wasi:io/poll). Current sync model covers 95% of use cases. Can add async bridge variant later.

## References

- [WAC (WebAssembly Composition)](https://github.com/bytecodealliance/wac)
- [WASI HTTP](https://github.com/WebAssembly/wasi-http)
- [Component Model](https://component-model.bytecodealliance.org/)
