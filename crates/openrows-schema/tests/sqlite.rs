// SPDX-License-Identifier: Apache-2.0
//! Schema cache happy path on the real `SQLite` driver (a temporary file).
mod support;

use openrows_driver_api::editor;
use support::{Place, TestResult, happy_path};

#[tokio::test]
async fn sqlite_listings_snapshot_search_and_refresh_after_ddl() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut session = openrows_driver_sqlite::open(&dir.path().join("cache.sqlite"), false, true)?;
    editor(&mut session)?
        .commit_ddl(&["CREATE TABLE invoices (id INTEGER PRIMARY KEY)".to_owned()])
        .await?;
    let place = Place {
        database: "main",
        schema: "main",
        table: "invoices",
    };
    happy_path(&mut session, &place).await
}
