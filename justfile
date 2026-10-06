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

# Measure five fresh release processes to the frontend paint-ready event.
bench-startup executable:
    python3 scripts/bench-startup.py "{{executable}}"
