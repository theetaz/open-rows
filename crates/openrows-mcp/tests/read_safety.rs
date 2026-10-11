// SPDX-License-Identifier: Apache-2.0
//! Local `SQLite` and libSQL file execution safety.
#[path = "support/read.rs"]
mod fixture;
use fixture::{exercise, file_request};
use openrows_driver_api::Driver;
use openrows_mcp::ReadEngine;
#[tokio::test]
async fn sqlite_and_libsql_file_refuse_attacks_with_zero_changes()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    for (driver, engine) in [
        (
            &openrows_driver_sqlite::SqliteDriver as &dyn Driver,
            ReadEngine::Sqlite,
        ),
        (
            &openrows_driver_sqlite::LibsqlFileDriver as &dyn Driver,
            ReadEngine::LibsqlFile,
        ),
    ] {
        let request = file_request(&directory.path().join(format!("{engine:?}.sqlite")));
        exercise(driver, &request, engine, "main").await?;
    }
    Ok(())
}
