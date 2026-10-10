// SPDX-License-Identifier: Apache-2.0
//! Native `DuckDB` fixture kept separate from the SQLite/probe helper.
#[path = "../../src/entry.rs"]
mod entry;

fn main() {
    entry::run(openrows_driver_host::run());
}
