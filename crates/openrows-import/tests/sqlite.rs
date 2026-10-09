// SPDX-License-Identifier: Apache-2.0
//! Imports into a real `SQLite` file through the driver's prepared-insert
//! bulk loader and batched `INSERT`s (runs in `just test`).
mod support;

use openrows_sql::Dialect;
use support::scenarios::{Engine, all};

const ENGINE: Engine = Engine {
    dialect: Dialect::Sqlite,
    schema: "main",
    columns: "(id INTEGER PRIMARY KEY, name TEXT, score REAL, active INTEGER, joined TEXT)",
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_import_scenario_passes_on_sqlite() -> support::TestResult {
    let dir = tempfile::tempdir()?;
    let mut connection = openrows_driver_sqlite::open(&dir.path().join("import.db"), false, true)?;
    all(&mut connection, &ENGINE).await
}
