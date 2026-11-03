# 29 — Status Snapshot and Test Guide

What is on `main`, what remains and how to test it, as of 2026-11-03. Use it to test the app and send feedback; feedback decides the order of the remaining work. [27 — Delivery plan](27-delivery-plan.md) keeps the per-task detail, and the [M1 exit audit](19-roadmap.md#m1-exit-audit-m1-32-2026-11-03) traces every P0 requirement to its test.

**Status:** alpha, macOS only, unsigned. The M1 exit is not met: no budget has been measured on the reference MacBook Air M1, and 8 of the 38 P0 requirements are partly proved.

## 1. Scope decision (2026-10-12)

OpenRows 1.0 is the **database client**. The API client (docs/11; roadmap M4 and the WebSocket, SSE, gRPC, collection runner, CLI and DB↔API items of M5) moves out of 1.0 and ships later as a separate product. Everything database-side stays in 1.0: TablePlus parity including every connection type, monitoring and analytics (M2.5), the database AI harness and MCP (M4.5), NoSQL (M3), the long-tail database drivers (M5) and Windows/Linux (M6).

Current priority: **test the app on `main` and collect feedback** before adding more engines or platforms.

## 2. On `main` and ready to test

| Area | What works | Engines |
| --- | --- | --- |
| Connections | Profiles, groups, environment tags, keychain secrets, URL ↔ form sync, SSH (password, key, agent, keyboard-interactive, jump hosts, `~/.ssh/config`), TLS modes and client certificates, SOCKS5/HTTP CONNECT proxies, AWS SSM, kubectl, Cloud SQL, cloudflared and custom-command hops, step-by-step Test connection, reconnect with keepalive, export/import of non-secret profiles | All |
| Cloud sign-in | **Cloud IAM token** in the connection sheet: AWS RDS/Aurora IAM (PostgreSQL, MySQL), Microsoft Entra ID (Azure Database for PostgreSQL and MySQL, Azure SQL) and Google Cloud SQL IAM (PostgreSQL, MySQL), through the `aws`, `az` and `gcloud` you already signed in to. A new token is fetched for every connect and reconnect and is never saved or shown. It is only sent over verified TLS, or for a Google token on PostgreSQL through the Cloud SQL Auth Proxy. The first connect asks you to approve the exact command and host. Tested with fake CLIs and a real local PostgreSQL only | Per provider |
| Engines | Tier 1: PostgreSQL (and compatible flavours), MySQL/MariaDB, SQLite. Tier 2 (beta): SQL Server, ClickHouse, DuckDB (driver host process), libSQL/Turso, MongoDB | — |
| MongoDB (beta) | `mongodb://` and `mongodb+srv://` connections (SRV, Direct connection, auth database) over direct, TLS, SSH and proxy routes; Collections/Views tree with indexes; MQL editor with shell-like syntax; query results as Table / Tree / JSON; collection tabs with whole-document editing (JSON/EJSON validation, review, single-transaction commit on a replica set); aggregation Pipeline editor with per-stage previews; Cancel; export; safe mode and read-only profiles. AI, Explain and monitoring are not available and say so | MongoDB 8 tested |
| Schema | Schema tree with cache and search, favourites and recents, structure/index/trigger/DDL views, routines, types, extensions and roles | All, capability-gated |
| Structure editing | Column/index/foreign-key/check sheets with DDL preview; primary-key and unique constraints, PostgreSQL `USING` casts and `CREATE INDEX CONCURRENTLY`, MySQL/MariaDB column positioning, SQL Server database rename; table/view/trigger/routine/database lifecycle editors | PG, MySQL/MariaDB, SQLite, SQL Server |
| SQL editor | CodeMirror editor with dialect highlighting and completion, run current/selection/all, cancel, multiple results (tabs or stacked), history, formatter, parameters bar, snippets, split panes, Explain/Explain Analyze plan view, indentation and completion settings | All |
| Grid and data editing | Canvas grid with Page Up/Down, Home/End, ⌘Home/⌘End, Tab/⇧Tab and ⌃Tab to leave; table browse with filters (AND/OR groups, raw predicates), any-column search, header quick filters, persisted striping/scrollbar/count settings; FK jumps, row form, bulk edit, inspector; HTML and configurable copy, date/time pickers; staged edits with review, single-transaction commit, undo/redo and crash recovery; a shadow ARIA grid that announces the focused cell and selection | All with keyed tables |
| Safety | Six safety policies including read-only and confirm-before-write, dangerous-statement confirmation, OS-authenticated app lock. The toolbar lock names the saved policy and opens its setting; the toolbar tag, strip and tab marks show the environment colour | All |
| Import/export | CSV/JSON/NDJSON import with mapping, run `.sql` files, CSV/JSON/SQL/xlsx/Parquet/Markdown/TSV export, `pg_dump`/`mysqldump` backup and restore | All, per engine |
| ER diagram | Read-only diagram with SVG/PNG export, diagram of selected tables, Web Worker layout, canvas rendering above 300 tables | All |
| Projects | File-backed saved queries in `.openrows/project.yaml`, shared non-secret connection definitions, snippets as project `.sql` files, saved-query keyword expansion and Insert/Copy SQL | — |
| Admin | Process list with cancel/kill, users and roles viewer | Per engine |
| Monitoring | Monitor workspace with live charts, query/lock investigation, deterministic alerts and incidents, analytics boards, health/analytics report export, optimisation advisor, Ask assistant panel. Opt-in history keeps the full 24 hours at the default 5 s cadence by rolling older samples into 20 s, 2 min and 4 min buckets | PG full; MySQL/MariaDB, SQL Server, ClickHouse, SQLite/libSQL, DuckDB per capability |
| AI | Off by default. Provider settings (Anthropic, OpenAI, OpenRouter, Gemini, compatible endpoints, Ollama, LM Studio, and the installed Codex CLI with its own ChatGPT sign-in and an isolated read-only child); API keys typed into a native secure macOS dialog; data-sharing policy with redaction and audit log; assistant panel with explain/fix/plan actions; inline ⌘I SQL proposals with accept/reject diff; read-only MCP server; your own MCP servers (stdio or HTTP) with per-tool enable and first-use confirmation; MCP servers declared in a project run only after you trust the exact definition; read-only monitoring tools behind telemetry consent, findings that cite evidence, board/report proposals saved only on accept, and opt-in scheduled read-only investigations with budgets | — |
| App | Settings (⌘,) with every section in a sidebar, themes, native menus, tabs, session restore, command palette, file associations; multiple windows (⇧⌘N, move tabs between windows from the tab menu, every window restored on relaunch); `openrows://` links with a confirmation sheet and Copy Link; `openrows-app open <link or file>` in a bundled app; ad-hoc signed macOS DMG and approval-gated update checks with an opt-out, inactive until the owner configures the release key. No telemetry: the crash-report switch in Settings ▸ Diagnostics is off and uploads nothing | — |

Visible English text comes from typed per-feature message catalogs, so a later translation needs no component change. English only: there is no language picker or second locale, and text from Rust and drivers and the grid and editor packages is not converted.

### Checks on `main` (2026-11-03)

Real results from the final run on the integrated `main`, on a local Mac. **GitHub CI is disabled and has run none of this. Nothing is pushed.**

| Command | Result |
| --- | --- |
| `just check` | All steps pass: fmt, clippy (`just lint`), deny, machete, hygiene, ui-check, bench-ui (342,582 of 350,000 gzip bytes), bench-test, script-test, release-test, eslint, prettier, tsc, vitest, knip, ipc-check |
| `just test` | 1,905 passed, 260 skipped, at 4 test threads (now the default in `.config/nextest.toml`, because 10 threads time out process-launching tests under their 5 s deadlines) |
| `just test-pg 17` | 97 passed, 4 skipped, plus the monitoring phases |
| `just test-mysql mysql:8.4` | 57 passed |
| `just test-mssql` | Passes on Azure SQL Edge |
| `just e2e` | Journeys 1–6 and 8 pass, with journey 4 at the macOS size of 10,000 rows |
| `just test-ssh` | 16 passed |
| `just test-mongo` | Passes on MongoDB 8 |
| `just test-clickhouse` | Passes on ClickHouse 25.3 |
| `just test-libsql` | Passes on libSQL server 0.24.32 |

Earlier the same day, on their work lanes: `just test-pg 13 15 16 17 18` (PG 16 has a known `pg_restore` 17.4 failure), `just test-mysql` on MySQL 8.4 and 9.7, MariaDB 11.4 and TiDB v8.5.1, and `just test-pg cockroach`.

## 3. Work-in-progress branches (not on `main`)

None.

## 4. Remaining work (not started)

| Group | Tasks |
| --- | --- |
| NoSQL | M3-05/06 Redis (key browser, type-aware editors, console, cluster/sentinel, pub/sub); M3-07 NoSQL conformance |
| Long-tail drivers | M5-01 Oracle (driver host + Instant Client), M5-02 Cassandra/ScyllaDB, M5-03 DynamoDB, M5-04 Elasticsearch/OpenSearch, M5-05 Redshift flavour and its IAM sign-in |
| App shell | Dragging tabs between windows and Merge All Windows (M5-11 follow-ups); `openrows://` registration on Windows/Linux (M6) |
| AI | Further installed-CLI provider bridges (docs/23), after provider approval and isolation validation |
| Conformance | Running the Tier 1 groups on Tier 2 engines; the other engines' wider version matrix |
| Windows and Linux | M6-01..05: credential backends, WebView2/WebKitGTK workarounds, packaging, E2E, per-OS capability matrix |
| Release hardening | Reference-hardware budget run (M0-S6b, M1-32), accessibility audit, documentation site, beta cycle |

Work that is started but not finished is in §5.

## 5. Known issues

**Performance budgets missed.** All measured on an M4 under load, not on the reference MacBook Air M1 (numbers in docs/13 §4 and the [budget table](19-roadmap.md#budgets-measured-so-far)):

- Completion popup p95 is 113–146 ms at the default 100 ms delay, against 50 ms; it is 20–35 ms at 0 ms. `just bench-editor` fails this gate.
- Monitor-open footprint grows 107 MiB against 32 MiB, mostly in WebKit. `just bench-memory --scenario monitor` fails.
- Grid flings stall: one 93–159 ms frame per launch in `just bench-grid`. The stall follows time since launch, not grid work, and its cause is unnamed.
- Warm startup p50 is 302–319 ms against the follow-up goal of about 290 ms (the 400 ms budget passes).

**Live checks only the owner can do:**

- Per-provider AI evaluations with your own keys (M45-09, docs/23 §17).
- Cloud sign-in on AWS, Azure and Google, including MySQL's cleartext step and the Azure SQL token login against real servers (§7, item 19).
- PostgreSQL managed-provider denial probes (RDS/Aurora, Cloud SQL, Azure); Supabase and Neon flavour runs.
- Real signing, notarisation, clean-machine install and update. M1-31 is on `main`: `just release-dmg` builds an ad-hoc signed DMG, and `release.yml` signs, notarises and publishes only once the owner adds the secrets in [docs/16 §8](16-platform-and-distribution.md#how-to-cut-a-release).
- A VoiceOver pass over the grid.
- The Linux E2E job on WebdriverIO 10, and the first CI runs of `coverage`, `bench` and `nightly`. CI is switched off (the `CI` workflow is disabled on GitHub since 2026-10-13); re-enable it with `gh workflow enable CI`.

**Behaviour limits:**

- Native key entry is macOS only; on other platforms the command answers `unsupported`.
- CockroachDB and TiDB can offer actions the server then rejects, because per-profile UI gates use the driver's claim (docs/05 §2.1).
- TiDB: a cancel that races a statement's end can lose the session, and `information_schema` omits table grants for an unprivileged login (docs/05 §13.1).
- Moving a project folder means trusting its MCP servers again, because trust is keyed to the project root. A changed MCP definition deletes its saved tokens (docs/23 §14).
- macOS text size is not followed (NFR-UX-03, M1-26): macOS offers no supported way to read it for an app like this, so font sizes stay in Settings ▸ Appearance. The owner decides whether to accept this.
- Toolbar Back and Forward are disabled placeholders.
- A production connection may be set to Unrestricted in the connection sheet. The toolbar shows that truthfully.
- Table page size is fixed at 300 rows, although FR-GRID-08 asks for a configurable size.
- FR-CONN-06 names a status bar, which the approved layout does not have. The colour is in the toolbar and on tabs; the owner decides whether to change the requirement's wording.
- Tier 2 engines are labelled beta. MongoDB has no E2E journey.
- Dependency advisories (checked 2026-11-02): `just deny` passes with an empty ignore list. `pnpm audit` has no high or critical findings; two moderate ones remain, in development tooling only and with no compatible patched release: `minimatch` (through ESLint 9, typescript-eslint and WebdriverIO) and `glob` 13 (through WebdriverIO).

**Open delivery rows:**

- M1-03 and M2-01: no Windows compile is recorded.
- M2-01: the driver host misses its 150 ms cold start (first raw handshake 1,110.587 ms; docs/27 §5).
- M1-05: no fuzz run is recorded (`just fuzz-smoke 60`, or the first `Nightly`).
- M45-09: live evaluations, above.
- M1-32: see the [M1 exit audit](19-roadmap.md#m1-exit-audit-m1-32-2026-11-03).

## 6. How to run it

Requirements and first-time setup: [CONTRIBUTING §1](../CONTRIBUTING.md#1-first-time-setup).

```sh
just setup        # once: check tools, install pnpm deps, enable git hooks
just dev-cert     # macOS, once: stable dev signing so keychain "Always Allow" sticks
just dev          # native window with hot reload
just build-app    # unsigned release app
just release-dmg  # ad-hoc signed DMG with the 40 MB installer check
just check        # every local quality gate, stopping at the first failure
just e2e          # native E2E journeys 1–6 and 8 against disposable PostgreSQL + OpenSSH containers (docs/17 §7)
just test-pg 13 15 16 17 18  # PostgreSQL suites and monitoring fixtures, one version (and one container set) at a time
just test-pg cockroach       # conformance suite on a CockroachDB single node
just test-mysql              # MySQL 8.4 and 9.7, MariaDB 11.4 and TiDB, one image at a time
just test-mssql              # SQL Server (Azure SQL Edge on arm64)
just test-mongo              # MongoDB on a replica set
just test-ssh                # OpenSSH matrix; fails instead of skipping without Docker
just test-clickhouse         # ClickHouse in a throwaway container
just test-libsql             # libSQL server in a throwaway container
```

A throwaway PostgreSQL for testing (remove it afterwards with `docker rm -f openrows-try-pg`):

```sh
docker run -d --rm --name openrows-try-pg -e POSTGRES_PASSWORD=openrows -p 127.0.0.1:55433:5432 postgres:17
# connect to postgres://postgres:openrows@127.0.0.1:55433/postgres
```

## 7. Test checklist

Work through these on your real databases; note anything that is wrong, slow, confusing or missing compared with TablePlus.

1. **Connect:** paste a URL and check the form fills; edit the form and check the URL; Test connection over direct, SSH and TLS; save, reopen and reconnect after sleep. Quit and reopen once to check a saved password comes from the Keychain, the one step no automated test records.
2. **Browse:** schema tree, search, favourites; structure, indexes, triggers and DDL views; ER diagram of a whole schema and of selected tables.
3. **Query:** run current/selection/all, cancel a long query, multiple results (try stacked results in Settings), history, formatter, completion settings, Explain plan. Check highlighting in a MySQL and a SQLite tab as well as PostgreSQL.
4. **Table data:** keyboard: Page Up/Down, Home/End, ⌘Home/⌘End, Tab/⇧Tab and ⌃Tab to leave the grid; filters, any-column search, header quick filters, sort, FK jump, row form; copy as TSV/CSV/JSON/HTML/SQL with custom delimiters. At a medium window size with the inspector open, check the footer's row range does not overlap "+ Row".
5. **Edit safely:** edit, insert, duplicate and delete rows; review the generated SQL; commit, then make a change that fails and check the whole commit rolls back; undo/redo; date/time pickers.
6. **Safe mode:** on a read-only connection, the toolbar lock names the policy. In the editor, the grid and the command palette (Run current statement, Insert row, Commit changes), every write is blocked and the rows stay unchanged. On a confirm-before-write connection, writes prompt; dangerous statements need confirmation. Click the toolbar lock: it opens the connection's settings with the policy focused, and nothing changes until you save. Set a production connection to Unrestricted and check the toolbar says so.
7. **Environment colour:** for each environment tag, check the tag, the strip under the toolbar, the tab marks and the tab tooltip, in light, dark and increased contrast. Try a custom tag with its own colour.
8. **Reconnect:** with a credential command, sleep the Mac with several databases open. All of them reconnect, and each run of your command is fresh.
9. **Structure:** add a primary key or unique constraint, change a column type (PostgreSQL `USING`), add an index concurrently, reorder a MySQL column, check the DDL preview before applying.
10. **Import/export and backup:** import a CSV, export to xlsx/Parquet, run a `.sql` file, back up and restore with `pg_dump`.
11. **Monitoring:** open Monitor on PostgreSQL; check live charts, query and lock investigation, alerts, a board, and a report export. Quit with a Monitor open and check the app exits cleanly.
12. **Monitoring history:** enable history for a connection and leave the Monitor open for a few hours, then for a full day. The 6 h and 24 h ranges must draw continuous lines with gaps only where collection stopped.
13. **AI provider and native key dialog:** turn AI features on and add a provider. The key field is not in the window: enter the key in the system dialog that opens after saving, then try **Set API key…** and Cancel. Set the data-sharing policy, ask the assistant to explain or fix a query, and check the context preview and audit log. Press ⌘I in the editor, then accept one proposal and reject another.
14. **Assistant on monitoring:** in Monitor → Ask assistant (telemetry consent on), ask why a query is slow or what is blocking, check the cited evidence, accept and reject a board proposal, and create, run, pause and cancel a scheduled investigation.
15. **Codex CLI:** with the Codex CLI installed, add a **Codex CLI** provider, press Check status, sign in with ChatGPT, check status again and ask the assistant with that provider. Try a question that needs several tools and check it is not cut off at one minute.
16. **MCP servers:** add your own server in Settings → AI → MCP, enable a tool and use it (first-use confirmation; the trace reads "server → tool"). For **project trust**, add an `mcp_servers` entry to a project's `.openrows/project.yaml` (docs/23 §14), open the project and check it is listed as not trusted. Trust it, set its variable in the system dialog, discover and enable a tool, and use it from the assistant. Edit the file and reopen the project: the trust and the saved value are gone. Move the project folder: it asks for trust again.
17. **App, windows and links:** settings (every section listed in the sidebar at the smallest window size), themes, tabs, session restore after quitting, app lock. Open a second window (⇧⌘N), move a query tab with unsaved SQL and a table tab with staged edits to it, lock the app and check both windows lock, quit and relaunch to see both windows return. In a bundled app (`just build-app`), Copy Link on a connection, a table and a filtered table, run `open '<link>'` in Terminal, check the Open link? preview and that nothing connects before Connect and Open; try `OpenRows.app/Contents/MacOS/openrows-app open connection "<name>"`.
18. **MongoDB:** add a `mongodb://` connection (paste a URL; try Direct connection for a single member or a container) and connect directly and over SSH. Browse Collections and a collection's indexes, open a collection, edit a document (try invalid JSON first, then a valid change through Review & commit), run `db.<collection>.find({})` and switch Table / Tree / JSON, preview pipeline stages, cancel a long aggregation, export a result and a collection, and check that a read-only profile blocks writes and that AI and Monitor say they are not available.
19. **Cloud sign-in (M5-13, live cloud validation pending; needs your own accounts).** In each case, in the connection sheet set **Store: Cloud IAM token**, press **Test**, read the approval dialog (the program, arguments and database host must be yours) and choose Trust and run. Then Connect, run `SELECT 1`, and run `SELECT 1` again after at least 20 minutes and after sleeping the Mac: the reconnect must not ask you anything or fail.
    - **AWS RDS or Aurora PostgreSQL:** turn on IAM database authentication on the instance; run `CREATE USER app_iam; GRANT rds_iam TO app_iam;`; allow `rds-db:connect` for your identity; run `aws sso login` (or `aws configure`). In the sheet use the instance endpoint as Host, `app_iam` as User, TLS **Verify full** with **Cloud CA: AWS RDS / Aurora**, Provider **AWS RDS / Aurora IAM**, your Region and AWS profile.
    - **AWS RDS MySQL or Aurora MySQL:** the same, with `CREATE USER app_iam IDENTIFIED WITH AWSAuthenticationPlugin AS 'RDS';`. This also exercises the `mysql_clear_password` path, which no test here could reach on a real server.
    - **Azure Database for PostgreSQL and for MySQL:** enable Microsoft Entra authentication, create the Entra database user, run `az login`. Use TLS **Verify full** with **Cloud CA: Azure Database**, Provider **Microsoft Entra ID**, the Entra user as User, and Tenant if you have several.
    - **Azure SQL:** add an Entra user to the database, run `az login`. Use Encrypt **Mandatory** with Trust server certificate off; User may stay empty. If the server reports the "Redirect" policy, connect to the host it names or use the "Proxy" policy. **This is the first test of the token login against a real server.**
    - **Google Cloud SQL PostgreSQL and MySQL:** set `cloudsql.iam_authentication=on`, add your IAM user, run `gcloud auth login`. Use TLS **Verify CA** with the instance's `server-ca.pem` as the CA file, Provider **Google Cloud SQL IAM**, and User as your email on PostgreSQL or the part before `@` on MySQL. On PostgreSQL also try the Cloud SQL Auth Proxy route with TLS off.
    - **Failures to check:** sign out (`aws sso logout`, `az logout`, `gcloud auth revoke`) and Test: the error names the sign-in command. Set a wrong AWS profile, a region other than the endpoint's, or a wrong tenant: the error says to check them. Set TLS to Require or Prefer: the sheet warns and Test refuses before any CLI runs. Remove database permission from your identity: Test's Authenticate step explains the IAM setup. Export the connection and search the file for the token (it must not be there), then import it: the first connect asks for approval again.

Report each item as: what you did, what you expected, what happened, and a screenshot when it is visual.
