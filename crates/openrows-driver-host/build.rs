// SPDX-License-Identifier: Apache-2.0
//! Resolve desktop build features and bound macOS fixture dynamic-linking work.

#[allow(clippy::print_stdout, reason = "Cargo build-script directives")]
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    // These standalone fixtures load no plugins. Exporting native C++ weak
    // symbols makes dyld repeat expensive interposition before every handshake.
    // Keep the production sidecar's linker policy and fixture debug symbols.
    for fixture in ["host-test-fixture", "host-duckdb-fixture"] {
        println!("cargo::rustc-link-arg-bin={fixture}=-Wl,-no_exported_symbols");
    }
}
