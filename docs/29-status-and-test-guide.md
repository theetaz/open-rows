# 29 — Status Snapshot and Test Guide

Snapshot of what is on `main`, what is parked on work-in-progress branches, and what remains, as of 2026-10-13. Use it to test the app and send feedback; feedback now decides the order of the remaining work. [27 — Delivery plan](27-delivery-plan.md) keeps the per-task detail.

## 1. Scope decision (2026-10-12)

OpenRows 1.0 is the **database client**. The API client (docs/11; roadmap M4 and the WebSocket, SSE, gRPC, collection runner, CLI and DB↔API items of M5) moves out of 1.0 and ships later as a separate product. Everything database-side stays in 1.0: TablePlus parity including every connection type, monitoring and analytics (M2.5), the database AI harness and MCP (M4.5), NoSQL (M3), the long-tail database drivers (M5) and Windows/Linux (M6).

Current priority: **dogfood the app on `main` and collect feedback** before adding more engines or platforms.

## 2. On `main` and ready to test

| Area | What works | Engines |
| --- | --- | --- |
| Connections | Profiles, groups, environment tags, keychain secrets, URL ↔ form sync, SSH (password, key, agent, keyboard-interactive, jump hosts, `~/.ssh/config`), TLS modes and client certificates, SOCKS5/HTTP CONNECT proxies, AWS SSM, kubectl, Cloud SQL, cloudflared and custom-command hops, step-by-step Test connection, reconnect with keepalive, export/import of non-secret profiles | All |
| Engines | Tier 1: PostgreSQL (and compatible flavours), MySQL/MariaDB, SQLite. Tier 2 (beta): SQL Server, ClickHouse, DuckDB (driver host process), libSQL/Turso, MongoDB | — |
| MongoDB (beta) | **new:** `mongodb://` and `mongodb+srv://` connections (SRV, Direct connection, auth database) over direct, TLS, SSH and proxy routes; Collections/Views tree with indexes; MQL editor with shell-like syntax; query results as Table / Tree / JSON; collection tabs with whole-document editing (JSON/EJSON validation, review, single-transaction commit on a replica set); aggregation Pipeline editor with per-stage previews; Cancel; export; safe mode and read-only profiles. AI, Explain and monitoring are not available and say so | MongoDB 8 tested |
| Schema | Schema tree with cache and search, favourites and recents, structure/index/trigger/DDL views, routines, types, extensions and roles | All, capability-gated |
| Structure editing | Column/index/foreign-key/check sheets with DDL preview; **new:** add primary-key and unique constraints, PostgreSQL `USING` casts and `CREATE INDEX CONCURRENTLY`, MySQL/MariaDB column positioning, SQL Server database rename; table/view/trigger/routine/database lifecycle editors | PG, MySQL/MariaDB, SQLite, SQL Server |
| SQL editor | CodeMirror editor with dialect highlighting and completion, run current/selection/all, cancel, multiple results, history, formatter, parameters bar, snippets, split panes, Explain/Explain Analyze plan view; **new:** editor indentation settings, completion qualification/closing-delimiter/acceptance-key settings, stacked results | All |
| Grid and data editing | Canvas grid, table browse with filters (AND/OR groups, raw predicates), FK jumps, row form, bulk edit, inspector; staged edits with review, single-transaction commit, undo/redo and crash recovery; **new:** any-column search, header quick filters, persisted striping/scrollbar/count settings, HTML and configurable copy, date/time pickers | All with keyed tables |
| Safety | Read-only and confirm-before-write profiles, dangerous-statement confirmation, safety policies, OS-authenticated app lock | All |
| Import/export | CSV/JSON/NDJSON import with mapping, run `.sql` files, CSV/JSON/SQL/xlsx/Parquet/Markdown/TSV export, `pg_dump`/`mysqldump` backup and restore | All, per engine |
| ER diagram | Read-only diagram with SVG/PNG export; **new:** diagram of selected tables, Web Worker layout, canvas rendering above 300 tables | All |
| Projects | File-backed saved queries in `.openrows/project.yaml`; **new:** shared non-secret connection definitions, snippets as project `.sql` files, saved-query keyword expansion and Insert/Copy SQL | — |
| Admin | Process list with cancel/kill, users and roles viewer | Per engine |
| Monitoring | Monitor workspace with live charts and history, query/lock investigation, deterministic alerts and incidents, analytics boards, health/analytics report export, optimisation advisor; **new:** Ask assistant panel for AI investigations of slow queries, locks and health with cited evidence | PG full; MySQL/MariaDB, SQL Server, ClickHouse, SQLite/libSQL, DuckDB per capability |
| AI | Provider settings (Anthropic, OpenAI, OpenRouter, Gemini, compatible endpoints, Ollama, LM Studio, and **new:** the installed Codex CLI through its official app-server with its own ChatGPT sign-in, explicit not installed/signed out/signed in/unsupported states and an isolated read-only child), data-sharing policy with redaction and audit log, assistant panel with explain/fix/plan actions, read-only MCP server; **new:** inline ⌘I SQL proposals with accept/reject diff (also in the editor bar's AI menu), and your own MCP servers (stdio or HTTP) in Settings → AI → MCP with per-tool enable and first-use confirmation; **new:** read-only monitoring tools (health, history, query statistics, locks, plans) behind telemetry consent, grounded findings that cite evidence, board/report proposals saved only on accept (invalid ones rejected with a reason), and opt-in scheduled read-only investigations with budgets, Run now, Pause/Resume and Cancel that pause while locked or with AI off; **new:** API keys are typed into a native secure macOS dialog (Set API key…) and never pass through the app window, MCP servers declared in a project's `project.yaml` run only after you trust the exact definition (a changed file removes the trust), and the Codex turn deadline is 5 minutes by default (60–900 s in Settings → AI) | — |
| App | Settings (⌘,), themes, native menus, tabs, session restore, command palette, file associations; **new:** macOS DMG packaging and signed update checks with release notes, approval-gated install and an opt-out in Settings ▸ General (inactive until the owner configures the release key); **new:** multiple windows (Window ▸ New Window ⇧⌘N, move tabs between windows from the tab menu, every window restored on relaunch), `openrows://` links with a confirmation sheet and Copy Link, and `openrows-app open <link or file>` (needs a bundled app: `just build-app`) | — |

**Polish (2026-11-02):** the grid adds Page Up/Down (scrolls with the focus, ⇧ extends), Home/End and ⌘/Ctrl+Home/End, Tab/⇧Tab across cells with row wrapping, and ⌃Tab to leave the grid, also at 10 M rows (docs/08 §4); Settings lists its sections in a sidebar so all of them fit at the sheet's minimum width; assistant traces name external MCP tools "server → tool" instead of `user_0_0` (docs/23).

**Monitoring follow-up (2026-11-02, M25-09/M25-02):** opt-in Monitor history now keeps the full 24 hours at the default 5 s cadence: each write rolls older samples into 20 s, 2 min and 4 min buckets (store migration 0009) that keep minimum, maximum, mean, last value and gaps, instead of evicting everything older than about two hours. A history tick with an unknown metric name keeps its other metrics. `just test-pg` takes several versions (`just test-pg 13 15 16 17 18`, one at a time) and adds PostgreSQL monitoring fixtures for temp spill, vacuum, statement and I/O statistics resets, an installed but not preloaded `pg_stat_statements`, and replication lag on a real primary/replica pair; all pass on 13, 15, 16, 17 and 18.

**Cloud IAM sign-in (2026-11-02, M5-13; live cloud validation pending):** Password storage in the connection sheet adds **Cloud IAM token**: AWS RDS/Aurora IAM (PostgreSQL, MySQL), Microsoft Entra ID (Azure Database for PostgreSQL and MySQL, Azure SQL) and Google Cloud SQL IAM (PostgreSQL, MySQL), using the `aws`, `az` and `gcloud` you already signed in to. A new token is fetched for every connect and reconnect and is never saved or shown. It is only sent over verified TLS, or for a Google token on PostgreSQL through the Cloud SQL Auth Proxy; other routes are refused with what to change. The first connect asks you to approve the exact command and database host. Only the provider, region, AWS profile, tenant or account are stored, and they travel in connection exports. Settings ▸ Tools also lists `az` and `gcloud`. No cloud account was available: tests use fake CLIs and a real local PostgreSQL, and the Azure SQL token login is checked against the protocol specification only. Redshift IAM waits for the Redshift flavour (M5-05).

**Fixed in this round:** boards, incidents, background monitoring and report saving were exported to the frontend but not registered with Tauri, so the app denied those calls; they are now registered and allowed.

**Performance, accessibility and CI tooling (2026-11-02, lane D):** the grid's shadow ARIA grid now exposes the column header and the focused row between its neighbours (row headers from the row-number gutter) and announces selection and range changes politely, throttled to one message per 200 ms ("Row 12, name: Alice", "Selected 3 rows by 2 columns"; docs/08 §8). The table footer no longer lets the row range overlap "+ Row" at medium widths (for example a 1,600 px window with the inspector open): labels hide below a 900 px footer and the range never spills over its neighbours. Quitting the app with a Monitor open no longer panics in the exit handler ("there is no reactor running"). `just bench-editor` works again (auto-closing brackets broke its typing script) and also measures the completion popup; `just bench-memory --scenario monitor` measures the Monitor's footprint in an isolated home folder; `just coverage`, the CI `coverage` and `bench` jobs and the scheduled `Nightly` fuzzing workflow are in place (docs/17, docs/18 §4.1) but have not run on GitHub while CI is off.

**Localisation-ready strings (2026-11-03, M5-12):** visible English text now comes from typed per-feature message catalogs instead of components, so a later translation needs no component change; the app looks and reads as before. English only: there is no language picker or second locale. New hard-coded text fails `pnpm lint` (docs/07 §10). Text from Rust and drivers and the grid and editor packages are not converted.

## 3. Work-in-progress branches (not on `main`)

None. The last parked lane, `wip/mongodb` (M3-01..M3-04), was finished and merged on 2026-11-02 (MongoDB row in §2).

## 4. Remaining work after feedback (not started)

| Group | Tasks |
| --- | --- |
| NoSQL | M3-05/06 Redis (key browser, type-aware editors, console, cluster/sentinel, pub/sub); M3-07 NoSQL conformance |
| Long-tail drivers | M5-01 Oracle (driver host + Instant Client), M5-02 Cassandra/ScyllaDB, M5-03 DynamoDB, M5-04 Elasticsearch/OpenSearch, M5-05 Redshift flavour |
| Connections | M5-13 live cloud validation (owner steps in §7, item 14); Redshift IAM once the Redshift flavour exists (M5-05) |
| App shell | Dragging tabs between windows and Merge All Windows (M5-11 follow-ups); `openrows://` registration on Windows/Linux (M6) |
| AI | Claude Code, Gemini CLI and Cursor bridges (after provider approval and isolation validation); live NL→SQL evaluation runs per provider/model with your own keys (docs/23 §17) to finish the M4.5 exit |
| Conformance and monitoring | Tier 1 conformance (M1-10/11/12/17) is done (2026-11-02); live Supabase and Neon flavour runs need accounts; running the Tier 1 groups on Tier 2 engines; M25 PostgreSQL managed-provider denial probes (need RDS/Aurora, Cloud SQL, Azure and Supabase/Neon accounts) and the other engines' wider version matrix |
| Windows and Linux | M6-01..05: credential backends, WebView2/WebKitGTK workarounds, packaging, E2E, per-OS capability matrix; M2-01 Windows MSVC host build |
| Release hardening | Reference-hardware budgets (startup, memory, driver-host cold start ≤ 150 ms), monitoring overhead, accessibility audit, documentation site, beta cycle |
| Performance follow-ups (2026-11-02) | Monitor view footprint (107 MiB increment against 32 MiB, WebKit GPU/WebContent); default completion delay decision (popup p95 113–146 ms at 100 ms, 20–35 ms at 0 ms); name the ≈ 100–160 ms main-thread block ≈ 30 s after launch that fails `just bench-grid` (Instruments trace), then fix it or add a settle period (docs/13 §4) |

## 5. Known issues

- **FR-CONN-06 names a status bar**, which the approved layout does not have. The environment colour is in the toolbar and on tabs (docs/20 "Environment colour"); the owner decides whether to change the requirement's wording.
- **OS text size is not followed** (NFR-UX-03): macOS offers no supported way to read it for an app like this, so font sizes stay in Settings ▸ Appearance (evidence in docs/20 "OS text size"). The owner decides whether to accept this.
- **A production connection can be set to Unrestricted** in the connection sheet. The toolbar now shows that truthfully; docs/20 used to say safe mode is always on for production.
- Tier 2 engines are labelled beta. SQL Server exact `float`/`real` edit guards need the CLR, which Azure SQL Edge lacks.
- No signed build yet. M1-31 release engineering is on `main`: `just release-dmg` builds an ad-hoc signed DMG, and `release.yml` signs, notarises and publishes only once the owner adds the Apple and updater secrets listed in [docs/16 §8](16-platform-and-distribution.md#how-to-cut-a-release); without them it skips those stages with a notice. Real signing, notarisation and a fresh-machine install and update test are still open.
- `just bench-grid` fails on `main` (one 93–159 ms frame per launch): a main-thread block about 30 s after launch, not grid work (docs/13 §4, "M0 S2 grid flings stall"). `just bench-editor` fails its new completion gate at the default 100 ms completion delay, and `just bench-memory --scenario monitor` fails the Monitor increment; both are recorded with numbers in docs/13 §4.
- Dependency advisories (checked 2026-11-02): `just deny` passes with an empty ignore list. `pnpm audit` has no high or critical findings; two moderate ones remain, in development tooling only and with no compatible patched release: `minimatch` (through ESLint 9, typescript-eslint and WebdriverIO) and `glob` 13 (through WebdriverIO). WebdriverIO moved to 10 because 9 pulled in `extract-zip`, which has no patched release; the Linux `e2e` job has not yet run on it, only a check of its calls against a stub WebDriver server.
- **CI is switched off** (the `CI` workflow is disabled on GitHub, 2026-10-13) while a single maintainer dogfoods the app; re-enable it with `gh workflow enable CI`. Earlier runs also could not start because of a GitHub billing/spending-limit problem.
- The 2026-10-13 merges were not run through the full `just check` or the container suites on the integrated `main`; each lane ran focused checks only. Run `just check` before re-enabling CI.

## 6. How to run it

```sh
just setup        # once: tools, toolchain, pnpm deps, git hooks
just dev-cert     # macOS, once: stable dev signing so keychain "Always Allow" sticks
just dev          # native window with hot reload
just build-app    # unsigned release app
just release-dmg  # ad-hoc signed DMG with the 40 MB installer check
just e2e          # native E2E journeys 1–6 and 8 against disposable PostgreSQL + OpenSSH containers (docs/17 §7)
just test-pg 13 15 16 17 18  # PostgreSQL suites and monitoring fixtures, one version (and one container set) at a time
just test-pg cockroach       # conformance suite on a CockroachDB single node
just test-mysql              # MySQL 8.4 and 9.7, MariaDB 11.4 and TiDB, one image at a time
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
4. **Table data:** keyboard: Page Up/Down, Home/End, ⌘Home/⌘End, Tab/⇧Tab and ⌃Tab to leave the grid; filters, any-column search, header quick filters, sort, FK jump, row form; copy as TSV/CSV/JSON/HTML/SQL with custom delimiters.
5. **Edit safely:** edit, insert, duplicate and delete rows; review the generated SQL; commit, then make a change that fails and check the whole commit rolls back; undo/redo; date/time pickers.
6. **Safe mode:** a read-only profile blocks writes everywhere (editor, grid, palette); confirm-before-write prompts; dangerous statements need confirmation. The toolbar lock names the connection's real policy; clicking it opens the connection's settings with the policy focused and changes nothing until you save. Also check the environment colour: tag, strip under the toolbar and tab marks for each environment, in light, dark and increased contrast. With a credential command, sleep the Mac with several databases open: all of them reconnect, and each run of your command is fresh.
7. **Structure:** add a primary key or unique constraint, change a column type (PostgreSQL `USING`), add an index concurrently, reorder a MySQL column, check the DDL preview before applying.
8. **Import/export and backup:** import a CSV, export to xlsx/Parquet, run a `.sql` file, back up and restore with `pg_dump`.
9. **Monitoring:** open Monitor on PostgreSQL; check live charts, history, query and lock investigation, alerts, a board, and a report export. With history enabled for a connection, leave the Monitor open for a few hours and check that the 6 h and 24 h ranges draw continuous lines with gaps only where collection stopped.
10. **AI:** add a provider and enter its key in the system dialog that opens after saving (also try **Set API key…** and Cancel), set the data-sharing policy, ask the assistant to explain or fix a query, check the context preview and audit log. In Monitor → Ask assistant (telemetry consent on), ask why a query is slow or what is blocking, check the cited evidence, accept and reject a board proposal, and create, run, pause and cancel a scheduled investigation. With the Codex CLI installed, add a **Codex CLI** provider, press Check status, sign in with ChatGPT, check status again and ask the assistant with that provider; try a question that needs several tools and check it is not cut off at one minute.
    Add an `mcp_servers` entry to a project's `.openrows/project.yaml` (docs/23 §14), open the project, and in Settings → AI → MCP check it is listed as not trusted; trust it, set its variable in the system dialog, discover and enable a tool, use it from the assistant (first-use confirmation), then edit the file and reopen the project to see the trust removed.
11. **App:** settings (every section listed in the sidebar at the smallest window size), themes, tabs, session restore after quitting, app lock.
12. **Windows and links:** open a second window (⇧⌘N), move a query tab with unsaved SQL and a table tab with staged edits to it, lock the app and check both windows lock, quit and relaunch to see both windows return. In a bundled app (`just build-app`), Copy Link on a connection, a table and a filtered table, run `open '<link>'` in Terminal, check the Open link? preview and that nothing connects before Connect and Open; try `OpenRows.app/Contents/MacOS/openrows-app open connection "<name>"`.

13. **MongoDB:** add a `mongodb://` connection (paste a URL; try Direct connection for a single member or a container) and connect directly and over SSH. Browse Collections and a collection's indexes, open a collection, edit a document (try invalid JSON first, then a valid change through Review & commit), run `db.<collection>.find({})` and switch Table / Tree / JSON, preview pipeline stages, cancel a long aggregation, export a result and a collection, and check that a read-only profile blocks writes and that AI and Monitor say they are not available.

14. **Cloud IAM (M5-13, live cloud validation pending; needs your own accounts).** In each case, in the connection sheet set **Store: Cloud IAM token**, press **Test**, read the approval dialog (the program, arguments and database host must be yours) and choose Trust and run. Then Connect, run `SELECT 1`, and run `SELECT 1` again after at least 20 minutes and after sleeping the Mac: the reconnect must not ask you anything or fail.
    - **AWS RDS or Aurora PostgreSQL:** turn on IAM database authentication on the instance; run `CREATE USER app_iam; GRANT rds_iam TO app_iam;`; allow `rds-db:connect` for your identity; run `aws sso login` (or `aws configure`). In the sheet use the instance endpoint as Host, `app_iam` as User, TLS **Verify full** with **Cloud CA: AWS RDS / Aurora**, Provider **AWS RDS / Aurora IAM**, your Region and AWS profile.
    - **AWS RDS MySQL or Aurora MySQL:** the same, with `CREATE USER app_iam IDENTIFIED WITH AWSAuthenticationPlugin AS 'RDS';`. This also exercises the `mysql_clear_password` path, which no test here could reach on a real server.
    - **Azure Database for PostgreSQL and for MySQL:** enable Microsoft Entra authentication, create the Entra database user, run `az login`. Use TLS **Verify full** with **Cloud CA: Azure Database**, Provider **Microsoft Entra ID**, the Entra user as User, and Tenant if you have several.
    - **Azure SQL:** add an Entra user to the database, run `az login`. Use Encrypt **Mandatory** with Trust server certificate off; User may stay empty. If the server reports the "Redirect" policy, connect to the host it names or use the "Proxy" policy. **This is the first test of the token login against a real server.**
    - **Google Cloud SQL PostgreSQL and MySQL:** set `cloudsql.iam_authentication=on`, add your IAM user, run `gcloud auth login`. Use TLS **Verify CA** with the instance's `server-ca.pem` as the CA file, Provider **Google Cloud SQL IAM**, and User as your email on PostgreSQL or the part before `@` on MySQL. On PostgreSQL also try the Cloud SQL Auth Proxy route with TLS off.
    - **Failures to check:** sign out (`aws sso logout`, `az logout`, `gcloud auth revoke`) and Test: the error names the sign-in command. Set a wrong AWS profile, a region other than the endpoint's, or a wrong tenant: the error says to check them. Set TLS to Require or Prefer: the sheet warns and Test refuses before any CLI runs. Remove database permission from your identity: Test's Authenticate step explains the IAM setup. Export the connection and search the file for the token (it must not be there), then import it: the first connect asks for approval again.

Report each item as: what you did, what you expected, what happened, and a screenshot when it is visual.
