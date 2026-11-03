# OpenRows

> A free, open-source database client for macOS. Alpha, unsigned.

OpenRows is a community-built database GUI (Apache-2.0) with a Rust core and a Tauri desktop shell. It sends no telemetry.

## Status

**Alpha, macOS only, unsigned.** There is no signed release yet, so build it from source. Windows and Linux are planned. The alpha exit is not met: performance budgets have not been measured on the reference Mac, and some requirements are only partly proved. Current state, known issues and a test checklist are in [docs/29](docs/29-status-and-test-guide.md); the requirement-by-requirement audit is in [docs/19](docs/19-roadmap.md#m1-exit-audit-m1-32-2026-11-03).

## Engines

| Support | Engines |
| --- | --- |
| Tier 1 | PostgreSQL and compatible flavours, MySQL/MariaDB, SQLite |
| Tier 2 (beta) | SQL Server, ClickHouse, DuckDB, libSQL/Turso, MongoDB |
| Not built yet | Redis, Oracle, Cassandra/ScyllaDB, DynamoDB, Elasticsearch/OpenSearch, Redshift |

Features vary by engine, and the app says when one is unavailable. CockroachDB and TiDB can offer actions the server then rejects.

## What it does

- **Connections:** groups, environment colours, URL ↔ form sync and step-by-step Test connection. Passwords stay in the macOS Keychain. Routes include SSH with jump hosts, TLS, SOCKS5 and HTTP proxies, and cloud hops. Cloud IAM sign-in is available through `aws`, `az` and `gcloud`, not yet tested against real cloud accounts.
- **Schema:** cached, searchable tree; structure, index, trigger and DDL views; structure editing with DDL preview; read-only ER diagrams.
- **SQL editor:** highlighting and completion, run current/selection/all, cancel, multiple results, history, formatter, parameters, snippets, split panes and Explain plans.
- **Data:** canvas grid with filters and inspector; edits staged for review, committed in one transaction, with undo/redo and crash recovery.
- **Safety:** read-only and confirm-before-write policies, dangerous-statement confirmation and an app lock.
- **Import and export:** CSV/JSON/NDJSON import, running `.sql` files, CSV/JSON/SQL/xlsx/Parquet/Markdown/TSV export, and backup and restore through your installed `pg_dump` and `mysqldump`.
- **Monitoring:** live charts, query and lock investigation, alerts, boards, reports, an advisor and opt-in 24-hour history.
- **AI (off by default):** bring your own provider or a local model, or use the installed Codex CLI. Includes a data-sharing policy, an assistant panel, ⌘I SQL proposals, a read-only MCP server and your own MCP servers.
- **App:** multiple windows, `openrows://` links, command palette, rebindable shortcuts, themes and session restore.

The API client planned earlier is out of 1.0 and will ship as a separate product.

## Build and run

You need macOS, Node.js ≥ 24, pnpm 12.9.1, `rustup` with Rust 1.94.0, `just`, `cargo-nextest`, `cargo-deny` and `cargo-machete`. Install commands are in [CONTRIBUTING](CONTRIBUTING.md#1-first-time-setup).

```bash
just setup        # check tools, install pnpm deps, enable git hooks
just dev-cert     # once: stable dev signing so keychain "Always Allow" sticks
just dev          # native window with hot reload
just build-app    # unsigned release app
just release-dmg  # ad-hoc signed DMG
```

`just check` runs every local quality gate. Container suites need Docker: `just test-pg`, `just test-mysql`, `just test-mssql`, `just test-mongo` and `just e2e`. See [docs/29 §6](docs/29-status-and-test-guide.md#6-how-to-run-it).

GitHub CI is currently disabled, so these checks run locally.

## Tech at a glance

| Layer | Choice |
| --- | --- |
| Core engine | Rust (tokio), one crate per concern, one crate per driver |
| App shell | Tauri 2 (system webview) |
| UI | React + TypeScript + Vite, CodeMirror 6, canvas-rendered data grid |
| Result transport | Apache Arrow IPC (binary, columnar), viewport-driven paging |
| Secrets | macOS Keychain |
| License | Apache-2.0 |

Design documentation starts at [docs/README.md](docs/README.md).

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md) before opening a pull request. In short: Conventional Commits, squash-merged PRs with Conventional Commit titles, docs updated with every change.

Also see [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) and [SECURITY.md](SECURITY.md).

## License

Apache License 2.0 — see [LICENSE](LICENSE) and [NOTICE](NOTICE). Licensing policy: [docs/22-governance-and-licensing.md](docs/22-governance-and-licensing.md).
