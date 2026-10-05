# 01 — Vision and Scope

## 1. Problem

Developers, data engineers and DBAs spend hours a day in database GUIs. The good ones are fast and polished but paid or subscription-based. The free ones tend to fall into two camps:

- **Heavy Java/Electron suites** — feature-rich but slow to start, memory hungry and visually dated.
- **Lightweight single-database tools** — pleasant but narrow; you end up with five apps for five databases.

API clients have the same split: the popular ones push cloud accounts and sync, the open ones are often missing protocols or polish.

## 2. Vision

> One small, fast, beautiful, local-first app to talk to every database and every API you work with — free and open source, forever.

### Product principles

1. **Speed is a feature.** Start in under a second. Never block typing or scrolling. Show the first rows before the last ones arrive.
2. **Safe by default.** Edits are staged and previewed before they hit the database. Production connections are visibly marked and guarded.
3. **Local-first, no accounts.** Everything works offline. Nothing leaves the machine unless the user sends it to their own database or API.
4. **Plain files you own.** Saved queries, API collections and workspace settings are human-readable text that diffs well in git.
5. **Native feel.** Respect platform conventions: menus, shortcuts, keychain, dark mode, window behaviour.
6. **Keyboard-first, mouse-friendly.** Every action reachable from the keyboard and a command palette; nothing *requires* the keyboard.
7. **One engine, many surfaces.** The same Rust core powers the desktop app and a headless CLI.
8. **Clean-room originality.** We learn from the category, never copy proprietary code, assets or text. See [22-governance-and-licensing.md](22-governance-and-licensing.md).

## 3. Users

| Persona | Needs | What delights them |
| --- | --- | --- |
| **App developer** | Quick look at tables, fix a row, run a query from a ticket, test an endpoint | Instant startup, edit-in-grid with preview, API + DB in one window |
| **Backend / platform engineer** | Many connections across environments, SSH bastions, query plans | Environment colour tags, SSH jump hosts, explain visualiser, prod read-only mode |
| **Data engineer / analyst** | Large result sets, warehouses, export to CSV/Parquet | Streaming millions of rows smoothly, fast export, DuckDB/ClickHouse support |
| **DBA** | Sessions, locks, users/roles, backups | Process list, kill query, role editor, dump/restore wrappers |
| **API tester / QA** | Collections, environments, assertions, CI | Git-friendly collections, test scripts, CLI runner |

## 4. Scope

### 4.1 In scope for 1.0

**Database client**
- Connection manager with groups, environment tags (local/dev/staging/prod), colours, SSH tunnels (incl. jump hosts), TLS, and keychain-stored secrets.
- Supported databases by tier (details in [05-database-drivers.md](05-database-drivers.md)):
  - **Tier 1:** PostgreSQL (+ compatibles), MySQL/MariaDB (+ compatibles), SQLite.
  - **Tier 2:** SQL Server, ClickHouse, DuckDB, libSQL/Turso, MongoDB, Redis.
  - **Tier 3:** Oracle, Cassandra/ScyllaDB, DynamoDB, Elasticsearch/OpenSearch.
- Schema browser, object search, structure viewer/editor (columns, indexes, constraints, triggers).
- SQL editor with dialect-aware autocomplete, formatting, multi-statement execution, run selection/current statement, parameters.
- Canvas data grid: millions of rows, sorting, filtering, column resize/reorder/hide, cell/row/column selection, copy as TSV/CSV/JSON/SQL.
- Data editing with staged change sets, SQL preview and transactional commit.
- Document (MongoDB) and key-value (Redis) native views.
- Query history (searchable), saved queries/snippets.
- Explain plan visualiser (PostgreSQL, MySQL, SQL Server, SQLite first).
- Import (CSV, JSON, SQL) and export (CSV, TSV, JSON, NDJSON, SQL INSERT, Excel, Parquet, Markdown).
- ER diagram for a schema (read-only in 1.0).
- Process list / kill query (where the database supports it).

**API client**
- HTTP/1.1 and HTTP/2 REST with all body types, auth helpers (Basic, Bearer, API key, OAuth 2.0 flows, AWS SigV4, Digest), cookies, proxies, client certificates.
- GraphQL with schema introspection and autocomplete.
- WebSocket, Server-Sent Events, gRPC (unary + streaming, reflection or `.proto`).
- Collections as files, environments, secrets in keychain, pre-request/post-response JS scripts, assertions/tests.
- Import from Postman, Insomnia, Bruno, OpenAPI 3.x, HAR, cURL; export to cURL and code snippets.
- Collection runner in-app and as CLI for CI.

**AI assistant (native, bring your own)** — see [23-ai-assistant.md](23-ai-assistant.md)
- Natural language → SQL/MQL, explain, fix errors, optimise with plans, generate test data.
- Providers: API keys (Anthropic, OpenAI, Gemini, Together, Groq, Mistral, OpenRouter, Bedrock, Azure, any OpenAI-compatible), local models (Ollama, LM Studio), and the user's installed agent CLIs (Claude Code, Codex CLI, Cursor CLI, Gemini CLI) to reuse existing subscriptions.
- MCP out of the box: the app is an MCP server (safe, read-only DB tools) and an MCP client (user's servers).
- Per-connection data-sharing policy; the model can never write to a database without human approval.

**Networking** — see [24-networking-and-protocols.md](24-networking-and-protocols.md)
- Multi-hop routes: SSH jump hosts (multiplexed), SOCKS5/HTTP proxies, AWS SSM, Kubernetes port-forward, Cloud SQL/AlloyDB proxies, Cloudflare Access, custom tunnel commands.
- Proactive recovery after sleep/wake and network changes; per-hop diagnostics.

**Workspace UX**
- Connection rail for instant switching between open connections; browser-style context bar; Chrome-style tabs for tables and queries with back/forward history; global safe-mode toggle.

**Platforms**
- macOS 12+ (Apple Silicon and Intel) first. Windows 10/11 and mainstream Linux distros before 1.0 GA.

### 4.2 Out of scope for 1.0 (candidates for later)

- Cloud sync service or accounts (git is the sync story).
- Real-time multi-user collaboration.
- Visual query builder (drag-and-drop joins).
- Editable ER diagrams / schema design from scratch.
- Mobile or web-hosted versions.
- Warehouses via dedicated drivers (Snowflake, BigQuery, Redshift-specific features, Databricks) — planned through the ADBC bridge post-1.0.
- Mock servers and API documentation publishing.

### 4.3 Explicit non-goals

- Becoming a BI/dashboard tool.
- Shipping proprietary database client libraries inside our binaries.
- Collecting usage data by default.

## 5. Success measures

| Measure | Target at 1.0 |
| --- | --- |
| Cold start to usable window (M1 MacBook Air) | ≤ 800 ms p95 |
| Idle memory (one PG connection, one tab) | ≤ 200 MB total incl. webview processes |
| Scroll through 1 M-row result | 60 fps sustained (120 fps on ProMotion where possible) |
| Crash-free sessions (opt-in reports) | ≥ 99.8 % |
| Data-loss bugs in edit/commit path | Zero open; every one is a release blocker |
| Install size (macOS universal DMG) | ≤ 40 MB |

## 6. Competitive landscape (for orientation only)

We study the category to understand user expectations, not to copy. Categories of existing tools:

- Native paid GUIs (multi-database, very fast, polished).
- Open-source Java suites (very broad support, heavy).
- Open-source Electron/web GUIs (modern UI, mid performance).
- Single-database official tools.
- API clients: cloud-first commercial, and open file-based ones.

Our differentiation: native-class speed **and** broad support **and** an integrated API client **and** fully open source.
