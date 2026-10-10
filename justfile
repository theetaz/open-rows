# Run Cargo through a working sccache, restarting or bypassing it when unusable (docs/18 §2.2).
sccache_guard := "bash scripts/sccache-guard.sh"

# Verify installed tools and enable repository git hooks.
setup:
    bash scripts/check-tools.sh
    pnpm install
    if [ "$(git config --get core.hooksPath)" != scripts/git-hooks ]; then git config core.hooksPath scripts/git-hooks; fi

# Format all Rust sources.
fmt:
    cargo fmt --all

# Check Rust formatting without changing files.
fmt-check:
    cargo fmt --all -- --check

# Enforce workspace Rust and Clippy lints as errors.
lint:
    {{sccache_guard}} cargo clippy --workspace --all-targets --locked -- -D warnings

# Run isolated workspace unit tests with nextest.
test:
    {{sccache_guard}} cargo nextest run --workspace --locked

# Run the OpenSSH container matrix with timings; fails (not skips) when Docker is unavailable.
test-ssh:
    OPENROWS_REQUIRE_DOCKER=1 {{sccache_guard}} cargo nextest run -p openrows-net --locked --test ssh_container --no-capture

# Check dependency licences, advisories, bans and sources.
deny:
    cargo deny --locked check

# Detect unused Rust dependencies.
machete:
    cargo machete

# Enforce source file length and issue-linked reminders.
hygiene:
    bash scripts/check-hygiene.sh

# Run all quality gates in order, stopping at the first failure.
check:
    just fmt-check
    just lint
    just test
    just deny
    just machete
    just hygiene
    just ui-check
    just bench-ui
    just bench-test
    just script-test
    pnpm lint
    pnpm format:check
    pnpm typecheck
    pnpm test
    pnpm knip
    just ipc-check

# Remove Cargo build output (including the configured shared target).
clean:
    cargo clean

# On macOS, `just dev` runs the app through a Cargo runner that signs it with the stable
# "OpenRows Dev" identity (when present), so keychain "Always Allow" survives rebuilds.
# Set only for this recipe: tests, CI and other platforms never see it.
dev_runner := if os() == "macos" { "env CARGO_TARGET_" + uppercase(replace(arch() + "-apple-darwin", "-", "_")) + "_RUNNER=" + quote(justfile_directory() / "scripts/dev-sign-run.sh") } else { "" }

# Launch the native desktop shell with frontend hot reload.
dev:
    {{sccache_guard}} {{dev_runner}} pnpm --filter @openrows/desktop tauri dev

# One-time on macOS: create the self-signed "OpenRows Dev" code-signing identity for `just dev`.
dev-cert:
    bash scripts/dev-cert.sh

# Build the unsigned release macOS application in the shared Cargo target.
build-app:
    {{sccache_guard}} pnpm --filter @openrows/desktop tauri build --bundles app

# Regenerate the TypeScript command contract from Rust.
gen-ipc:
    {{sccache_guard}} cargo run -p openrows-app --bin gen-ipc --locked

# Compare regenerated IPC bindings without modifying the committed contract.
ipc-check:
    {{sccache_guard}} cargo nextest run -p openrows-app --locked bindings_are_fresh

# Measure release startup phases; requires build-app and a macOS GUI session.
bench-startup runs="10":
    python3 -B scripts/bench-startup.py "{{runs}}"

# Measure all-process idle physical footprint five seconds after first paint on macOS.
bench-memory runs="3":
    python3 -B scripts/bench-memory.py "{{runs}}"

# Test benchmark budgets, reports and cleanup without launching the app.
bench-test:
    python3 -B -m unittest discover -s scripts -p 'test_bench_*.py'

# Test repository shell tooling against stubbed commands.
script-test:
    python3 -B -m unittest discover -s scripts -p 'test_sccache_*.py'

# Emit CSS custom properties from the UI TypeScript token source.
gen-tokens:
    pnpm --filter @openrows/ui gen-tokens

# Reject stale generated tokens and raw component colours.
ui-check:
    pnpm --filter @openrows/ui tokens:check
    pnpm --filter @openrows/ui colours:check

# Print minimum WCAG ratios across all component surfaces and themes.
contrast-report:
    pnpm --filter @openrows/ui contrast:report

# Build and enforce gzip budgets for UI styles and the initial JS bundle.
bench-ui:
    pnpm build
    pnpm --filter @openrows/ui size:report

# Measure 500 Arrow IPC tiles in WKWebView; builds nothing, maintainer-only GUI run.
bench-ipc:
    python3 -B scripts/bench-ipc.py

# Benchmark result slicing + encoding, then gate the 256x20 mean upper bound at 1 ms.
bench-result-encode:
    {{sccache_guard}} cargo bench -p openrows-results --bench result_window_encode --locked
    python3 -B scripts/bench-result-encode.py

# Benchmark PostgreSQL binary decoding, then gate >= 1 M cells/s/core and first batch <= 5 ms.
bench-pg-decode:
    {{sccache_guard}} cargo bench -p openrows-driver-postgres --bench decode_pg --locked
    python3 -B scripts/bench-pg-decode.py

# Benchmark SQL statement splitting (10 KB and 1 MB scripts per dialect), reported in bytes/s.
bench-sql-split:
    {{sccache_guard}} cargo bench -p openrows-sql --bench sql_split --locked

# Benchmark the schema cache at 50 000 objects, then gate first level <= 300 ms and search <= 50 ms.
bench-schema:
    {{sccache_guard}} cargo bench -p openrows-schema --bench schema_catalog --locked
    python3 -B scripts/bench-schema.py

# Run PostgreSQL integration tests in a throwaway container; skips when Docker is unavailable.
test-pg version="17":
    {{sccache_guard}} bash scripts/test-pg.sh "{{version}}"

# Run MySQL/MariaDB integration tests in throwaway containers (default mysql:8.4 and mariadb:11.4); skips when Docker is unavailable.
test-mysql *images:
    {{sccache_guard}} bash scripts/test-mysql.sh {{images}}

# Run SQL Server integration tests and conformance in a throwaway container (default mcr.microsoft.com/mssql/server:2022-latest); skips when Docker is unavailable.
test-mssql image="mcr.microsoft.com/mssql/server:2022-latest":
    {{sccache_guard}} bash scripts/test-mssql.sh "{{image}}"

# Run the ClickHouse LTS HTTP/TLS integration and conformance suites.
test-clickhouse image="clickhouse/clickhouse-server:25.3.9.72":
    {{sccache_guard}} bash scripts/test-clickhouse.sh "{{image}}"

# Run authenticated libSQL/Turso HTTP integration and remote/file conformance.
test-libsql image="ghcr.io/tursodatabase/libsql-server:v0.24.32":
    {{sccache_guard}} bash scripts/test-libsql.sh "{{image}}"

# Run each cargo-fuzz target in fuzz/ briefly; skips when nightly or cargo-fuzz is missing.
fuzz-smoke seconds="20":
    {{sccache_guard}} bash scripts/fuzz-smoke.sh "{{seconds}}"

# Measure the accepted canvas grid with fresh scenarios per repeat; maintainer-only GUI run.
bench-grid repeats="3":
    python3 -B scripts/bench-grid.py "{{repeats}}"

# Measure CodeMirror keystroke-to-paint on 5000 SQL lines in WKWebView; maintainer-only GUI run.
bench-editor:
    python3 -B scripts/bench-editor.py
