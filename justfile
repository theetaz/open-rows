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
    cargo clippy --workspace --all-targets --locked -- -D warnings

# Run isolated workspace unit tests with nextest.
test:
    cargo nextest run --workspace --locked

# Run the OpenSSH container matrix with timings; fails (not skips) when Docker is unavailable.
test-ssh:
    OPENROWS_REQUIRE_DOCKER=1 cargo nextest run -p openrows-net --locked --test ssh_container --no-capture

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
    pnpm lint
    pnpm format:check
    pnpm typecheck
    pnpm test
    pnpm knip
    just ipc-check

# Remove Cargo build output (including the configured shared target).
clean:
    cargo clean

# Launch the native desktop shell with frontend hot reload.
dev:
    pnpm --filter @openrows/desktop tauri dev

# Build the unsigned release macOS application in the shared Cargo target.
build-app:
    pnpm --filter @openrows/desktop tauri build --bundles app

# Regenerate the TypeScript command contract from Rust.
gen-ipc:
    cargo run -p openrows-app --bin gen-ipc --locked

# Compare regenerated IPC bindings without modifying the committed contract.
ipc-check:
    cargo nextest run -p openrows-app --locked bindings_are_fresh

# Measure release startup phases; requires build-app and a macOS GUI session.
bench-startup runs="10":
    python3 -B scripts/bench-startup.py "{{runs}}"

# Measure all-process idle physical footprint five seconds after first paint on macOS.
bench-memory runs="3":
    python3 -B scripts/bench-memory.py "{{runs}}"

# Test benchmark budgets, reports and cleanup without launching the app.
bench-test:
    python3 -B -m unittest discover -s scripts -p 'test_bench_*.py'

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
    cargo bench -p openrows-results --bench result_window_encode --locked
    python3 -B scripts/bench-result-encode.py

# Benchmark PostgreSQL binary decoding, then gate >= 1 M cells/s/core and first batch <= 5 ms.
bench-pg-decode:
    cargo bench -p openrows-driver-postgres --bench decode_pg --locked
    python3 -B scripts/bench-pg-decode.py

# Run PostgreSQL integration tests in a throwaway container; skips when Docker is unavailable.
test-pg version="17":
    bash scripts/test-pg.sh "{{version}}"

# Measure the accepted canvas grid with fresh scenarios per repeat; maintainer-only GUI run.
bench-grid repeats="3":
    python3 -B scripts/bench-grid.py "{{repeats}}"
