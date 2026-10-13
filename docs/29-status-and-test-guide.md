# 29 — Status Snapshot and Test Guide

Snapshot of what is on `main`, what is parked on work-in-progress branches, and what remains, as of 2026-10-13. Use it to test the app and send feedback; feedback now decides the order of the remaining work. [27 — Delivery plan](27-delivery-plan.md) keeps the per-task detail.

## 1. Scope decision (2026-10-12)

OpenRows 1.0 is the **database client**. The API client (docs/11; roadmap M4 and the WebSocket, SSE, gRPC, collection runner, CLI and DB↔API items of M5) moves out of 1.0 and ships later as a separate product. Everything database-side stays in 1.0: TablePlus parity including every connection type, monitoring and analytics (M2.5), the database AI harness and MCP (M4.5), NoSQL (M3), the long-tail database drivers (M5) and Windows/Linux (M6).

Current priority: **dogfood the app on `main` and collect feedback** before adding more engines or platforms.

## 2. On `main` and ready to test

| Area | What works | Engines |
| --- | --- | --- |
| Connections | Profiles, groups, environment tags, keychain secrets, URL ↔ form sync, SSH (password, key, agent, keyboard-interactive, jump hosts, `~/.ssh/config`), TLS modes and client certificates, SOCKS5/HTTP CONNECT proxies, AWS SSM, kubectl, Cloud SQL, cloudflared and custom-command hops, step-by-step Test connection, reconnect with keepalive, export/import of non-secret profiles | All |
| Engines | Tier 1: PostgreSQL (and compatible flavours), MySQL/MariaDB, SQLite. Tier 2 (beta): SQL Server, ClickHouse, DuckDB (driver host process), libSQL/Turso | — |
| Schema | Schema tree with cache and search, favourites and recents, structure/index/trigger/DDL views, routines, types, extensions and roles | All, capability-gated |
| Structure editing | Column/index/foreign-key/check sheets with DDL preview; **new:** add primary-key and unique constraints, PostgreSQL `USING` casts and `CREATE INDEX CONCURRENTLY`, MySQL/MariaDB column positioning, SQL Server database rename; table/view/trigger/routine/database lifecycle editors | PG, MySQL/MariaDB, SQLite, SQL Server |
| SQL editor | CodeMirror editor with dialect highlighting and completion, run current/selection/all, cancel, multiple results, history, formatter, parameters bar, snippets, split panes, Explain/Explain Analyze plan view; **new:** editor indentation settings, completion qualification/closing-delimiter/acceptance-key settings, stacked results | All |
| Grid and data editing | Canvas grid, table browse with filters (AND/OR groups, raw predicates), FK jumps, row form, bulk edit, inspector; staged edits with review, single-transaction commit, undo/redo and crash recovery; **new:** any-column search, header quick filters, persisted striping/scrollbar/count settings, HTML and configurable copy, date/time pickers | All with keyed tables |
| Safety | Read-only and confirm-before-write profiles, dangerous-statement confirmation, safety policies, OS-authenticated app lock | All |
| Import/export | CSV/JSON/NDJSON import with mapping, run `.sql` files, CSV/JSON/SQL/xlsx/Parquet/Markdown/TSV export, `pg_dump`/`mysqldump` backup and restore | All, per engine |
| ER diagram | Read-only diagram with SVG/PNG export; **new:** diagram of selected tables, Web Worker layout, canvas rendering above 300 tables | All |
| Projects | File-backed saved queries in `.openrows/project.yaml`; **new:** shared non-secret connection definitions, snippets as project `.sql` files, saved-query keyword expansion and Insert/Copy SQL | — |
| Admin | Process list with cancel/kill, users and roles viewer | Per engine |
| Monitoring | Monitor workspace with live charts and history, query/lock investigation, deterministic alerts and incidents, analytics boards, health/analytics report export, optimisation advisor | PG full; MySQL/MariaDB, SQL Server, ClickHouse, SQLite/libSQL, DuckDB per capability |
| AI | Provider settings (Anthropic, OpenAI, OpenRouter, Gemini, compatible endpoints, Ollama, LM Studio), data-sharing policy with redaction and audit log, assistant panel with explain/fix/plan actions, read-only MCP server | — |
| App | Settings (⌘,), themes, native menus, tabs, session restore, command palette, file associations | — |

**Fixed in this round:** boards, incidents, background monitoring and report saving were exported to the frontend but not registered with Tauri, so the app denied those calls; they are now registered and allowed.

## 3. Work-in-progress branches (not on `main`)

Each branch is one squashable lane, based on `main` as of 2026-10-12 (`a9c45f51`). The work is implemented but **not yet verified**. To resume: merge `main` into the branch, finish the listed items, run `just check` plus the named container recipe, then squash-merge.

| Branch | Tasks | State | To finish |
| --- | --- | --- | --- |
| `wip/mongodb` | M3-01..M3-04 MongoDB: document result model and tree/JSON/table views, driver crate, MQL parser and fuzz target, document editing, aggregation pipeline editor | Largest lane (+8.9 k lines); cursor and cancel correctness reworked | `just test-mongo`, fuzz target build, full gate |
| `wip/ai-editor-mcp` | M45-06 inline ⌘I with accept/reject diff; M45-07 MCP client for user servers | Implemented; credential-change catalogue invalidation added | Full gate |
| `wip/ai-monitoring` | M45-10 monitoring investigation tools, M45-11 report/chart/board proposals, M45-12 scheduled read-only investigations | Implemented; `just test-pg` passed once | Full gate, ipc-check |
| `wip/e2e` | M1-29 dev-only automation IPC, macOS smoke suite, Linux `tauri-driver` E2E and CI job | Implemented; found the grid spacer bug below | Grid fix, local E2E run, full gate |
| `wip/release-macos` | M1-31 DMG bundle, Developer ID signing/notarisation from secrets, updater with opt-out, `release.yml`, Homebrew cask template | Implemented | Local unsigned DMG build, full gate; real signing needs Apple certificates as GitHub secrets |

## 4. Remaining work after feedback (not started)

| Group | Tasks |
| --- | --- |
| NoSQL | M3-05/06 Redis (key browser, type-aware editors, console, cluster/sentinel, pub/sub); M3-07 NoSQL conformance |
| Long-tail drivers | M5-01 Oracle (driver host + Instant Client), M5-02 Cassandra/ScyllaDB, M5-03 DynamoDB, M5-04 Elasticsearch/OpenSearch, M5-05 Redshift flavour |
| Connections | M5-13 cloud IAM auth (RDS/Aurora/Redshift IAM, Entra ID, Cloud SQL IAM) |
| App shell | M5-11 multiple windows, M5-09 `openrows://` URL handler and CLI open, M5-12 localisation-ready strings |
| AI | M45-04 Codex app-server bridge (eligible CLI adapters); M45-09 exit evaluation |
| Conformance and monitoring | M1-10/11/12/17 Tier 1 conformance facets (TLS/SSH, transactions, golden introspection, MySQL 9.x), M25 history downsampling and remaining PostgreSQL fixtures |
| Windows and Linux | M6-01..05: credential backends, WebView2/WebKitGTK workarounds, packaging, E2E, per-OS capability matrix; M2-01 Windows MSVC host build |
| Release hardening | Reference-hardware budgets (startup, memory, driver-host cold start ≤ 150 ms), monitoring overhead, accessibility audit, documentation site, beta cycle |

## 5. Known issues

- **Grid row limit:** WebKit clamps the scroll spacer to 33,554,428 px, so with about 1.3 M+ rows at default density the last rows cannot be scrolled to. The fix (cap the physical spacer and map scroll position to the logical row range) is planned on `wip/e2e`.
- Tier 2 engines are labelled beta. SQL Server exact `float`/`real` edit guards need the CLR, which Azure SQL Edge lacks.
- No signed build yet; `just build-app` produces an unsigned app.
- **CI is switched off** (the `CI` workflow is disabled on GitHub, 2026-10-13) while a single maintainer dogfoods the app; re-enable it with `gh workflow enable CI`. Earlier runs also could not start because of a GitHub billing/spending-limit problem.
- The 2026-10-13 merges were not run through the full `just check` or the container suites on the integrated `main`; each lane ran focused checks only. Run `just check` before re-enabling CI.

## 6. How to run it

```sh
just setup        # once: tools, toolchain, pnpm deps, git hooks
just dev-cert     # macOS, once: stable dev signing so keychain "Always Allow" sticks
just dev          # native window with hot reload
just build-app    # unsigned release app
```

A throwaway PostgreSQL for testing (remove it afterwards with `docker rm -f openrows-try-pg`):

```sh
docker run -d --rm --name openrows-try-pg -e POSTGRES_PASSWORD=openrows -p 127.0.0.1:55433:5432 postgres:17
# connect to postgres://postgres:openrows@127.0.0.1:55433/postgres
```

## 7. Test checklist

Work through these on your real databases; note anything that is wrong, slow, confusing or missing compared with TablePlus.

1. **Connect:** paste a URL and check the form fills; edit the form and check the URL; Test connection over direct, SSH and TLS; save, reopen and reconnect after sleep.
2. **Browse:** schema tree, search, favourites; structure, indexes, triggers and DDL views; ER diagram of a whole schema and of selected tables.
3. **Query:** run current/selection/all, cancel a long query, multiple results (try stacked results in Settings), history, formatter, completion settings, Explain plan.
4. **Table data:** filters, any-column search, header quick filters, sort, FK jump, row form; copy as TSV/CSV/JSON/HTML/SQL with custom delimiters.
5. **Edit safely:** edit, insert, duplicate and delete rows; review the generated SQL; commit, then make a change that fails and check the whole commit rolls back; undo/redo; date/time pickers.
6. **Safe mode:** a read-only profile blocks writes everywhere (editor, grid, palette); confirm-before-write prompts; dangerous statements need confirmation.
7. **Structure:** add a primary key or unique constraint, change a column type (PostgreSQL `USING`), add an index concurrently, reorder a MySQL column, check the DDL preview before applying.
8. **Import/export and backup:** import a CSV, export to xlsx/Parquet, run a `.sql` file, back up and restore with `pg_dump`.
9. **Monitoring:** open Monitor on PostgreSQL; check live charts, history, query and lock investigation, alerts, a board, and a report export.
10. **AI:** add a provider key, set the data-sharing policy, ask the assistant to explain or fix a query, check the context preview and audit log.
11. **App:** settings, themes, tabs, session restore after quitting, app lock.

Report each item as: what you did, what you expected, what happened, and a screenshot when it is visual.
