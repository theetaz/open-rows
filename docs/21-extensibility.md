# 21 — Extensibility

Plugins are a post-1.0 feature, but the architecture keeps the door open from day one so that adding them later does not require rewriting the core.

## 1. Extension points

| Extension point | Examples | Mechanism |
| --- | --- | --- |
| **Database driver** | Community drivers for niche databases | WASM component (pure protocol over host-provided sockets) or out-of-process driver binary speaking the driver-host protocol |
| **Exporter / importer** | Avro, XML, custom CSV dialects | WASM component receiving Arrow IPC batches |
| **Value renderer** | Custom display of a domain type (e.g. geohash → map link) | Declarative config + optional WASM formatter |
| **Theme** | Colour themes, editor themes | JSON token files (no code) |
| **Snippets / keymaps** | Shared team snippets, alternative keymaps | JSON files |
| **API auth provider** | Custom signing schemes | WASM component with request-in / request-out contract |

## 2. Why WASM components

- **Sandboxed by construction:** a plugin gets only the capabilities the host passes in (WASI preview 2 interfaces): e.g. a driver gets "open TCP connection to the host:port the user configured" and nothing else.
- **Language-agnostic:** plugins can be written in Rust, Go, C, Zig, etc. that compile to components.
- **Portable:** one artifact for macOS/Windows/Linux, arm64/x64.
- Runtime: `wasmtime` with fuel/epoch-based interruption and memory limits.

## 3. Driver plugin contract (preview)

The WIT interface mirrors the Rust `Driver`/`Connection` traits but in a message-oriented form:

```wit
package odb:driver@0.1.0;

interface driver {
  record connect-config { params: list<tuple<string, string>>, secrets: list<tuple<string, string>> }
  record column-meta { name: string, logical-type: string, source-type: string, nullable: bool }
  variant exec-event {
    result-begin(list<column-meta>),
    batch(list<u8>),          // Arrow IPC stream bytes
    result-end(u64),
    rows-affected(u64),
    notice(string),
  }

  resource connection {
    execute: func(sql: string, params: list<string>) -> result<_, string>;
    next-event: func() -> option<exec-event>;
    cancel: func();
    close: func();
  }

  info: func() -> string;     // JSON: id, name, capabilities, params schema
  connect: func(cfg: connect-config) -> result<connection, string>;
}
```

Host imports provide: TCP/TLS sockets restricted to the configured endpoint (the host performs TLS and SSH tunnelling so plugins never handle certificates or keys), logging, and clock.

## 4. Distribution and trust

- Plugins installed from a file or a URL the user provides; an optional community index (a git repo of manifests) may come later.
- Manifest declares id, version, author, requested capabilities, checksum, and optional signature.
- Install shows requested capabilities; plugins are disabled by default for prod-tagged connections until explicitly allowed.
- No plugin can access the keychain, the filesystem (except a private data dir), or other connections.

## 5. What we do now to stay plugin-ready

- Drivers depend only on `odb-driver-api` + `odb-types` + `odb-net` (no reaching into core).
- Driver-host protocol is already message-oriented and Arrow-based ([05-database-drivers.md](05-database-drivers.md#9-out-of-process-driver-host)).
- Exporters implemented against an `Exporter` trait consuming `RecordBatch` streams.
- Themes are token files from the start ([07-frontend-architecture.md](07-frontend-architecture.md#8-theming)).
- UI derives toolbars and tree nodes from driver capabilities, not from driver names.
