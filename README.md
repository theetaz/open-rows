# OpenDB Client

> Working name. A free, open-source, fast, native-feeling database client and API client for macOS, Windows and Linux.

OpenDB Client is a community-built alternative to paid database GUIs. It aims to match or beat them on speed, polish and reliability while staying fully open source (Apache-2.0), local-first and free of accounts, telemetry and paywalls.

## What it will do

- **Database client** — connect to PostgreSQL, MySQL/MariaDB, SQLite, SQL Server, Oracle, ClickHouse, DuckDB, MongoDB, Redis, Cassandra/ScyllaDB, DynamoDB, Elasticsearch/OpenSearch and their wire-compatible cousins (CockroachDB, TiDB, Neon, Supabase, …). Browse schemas, run queries, edit data safely, import/export, inspect query plans.
- **API client** — REST, GraphQL, WebSocket, Server-Sent Events and gRPC requests organised in plain-text collections that live in your git repo. Environments, scripting, tests and a CLI runner for CI.
- **One fast engine** — a Rust core does all the heavy lifting (drivers, SSH tunnels, result storage, HTTP). The UI is a thin, GPU-friendly viewport on top of it.

## Status

Pre-alpha. We are writing the design documentation first. Start at [docs/README.md](docs/README.md).

## Tech at a glance

| Layer | Choice |
| --- | --- |
| Core engine | Rust (tokio), one crate per concern, one crate per driver |
| App shell | Tauri 2 (system webview, small binary) |
| UI | React + TypeScript + Vite, CodeMirror 6, canvas-rendered data grid |
| Result transport | Apache Arrow IPC (binary, columnar), viewport-driven paging |
| Secrets | OS keychain (macOS Keychain, Windows Credential Manager, Secret Service) |
| License | Apache-2.0 |

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md) before opening a pull request. In short: Conventional Commits, squash-merged PRs with Conventional Commit titles, docs updated with every change. After cloning, enable the commit-message hook:

```bash
git config core.hooksPath scripts/git-hooks
```

Also see [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) and [SECURITY.md](SECURITY.md).

## License

Apache License 2.0 — see [LICENSE](LICENSE) and [NOTICE](NOTICE). Licensing policy: [docs/22-governance-and-licensing.md](docs/22-governance-and-licensing.md).
