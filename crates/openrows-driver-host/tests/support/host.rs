// SPDX-License-Identifier: Apache-2.0
//! Feature-enabled fixture whose name cannot collide with Tauri's production sidecar.
#[path = "../../src/entry.rs"]
mod entry;

fn main() {
    entry::run(openrows_driver_host::run_sqlite_fixture());
}
