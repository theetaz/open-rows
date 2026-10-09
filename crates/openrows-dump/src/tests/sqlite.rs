// SPDX-License-Identifier: Apache-2.0
use super::*;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn names(path: &Path) -> rusqlite::Result<Vec<String>> {
    let connection = Connection::open(path)?;
    let mut statement = connection.prepare("SELECT name FROM items ORDER BY id")?;
    statement
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()
}

#[test]
fn backs_up_and_restores_over_an_open_database() -> TestResult {
    let dir = tempfile::tempdir()?;
    let live = dir.path().join("live.sqlite");
    let open = Connection::open(&live)?;
    open.execute_batch(
        "PRAGMA journal_mode=WAL;
         CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT);
         INSERT INTO items (name) VALUES ('alpha'), ('beta');",
    )?;
    let backup = dir.path().join("backup.sqlite");
    let bytes = sqlite_backup(&live, &backup)?;
    assert!(bytes > 0);
    assert_eq!(names(&backup)?, ["alpha", "beta"]);
    open.execute_batch("DELETE FROM items; INSERT INTO items (name) VALUES ('gamma');")?;
    sqlite_restore(&backup, &live)?;
    // The connection that stayed open sees the restored rows.
    let restored: Vec<String> = open
        .prepare("SELECT name FROM items ORDER BY id")?
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    assert_eq!(restored, ["alpha", "beta"]);
    Ok(())
}

#[test]
fn refuses_to_overwrite_or_restore_garbage() -> TestResult {
    let dir = tempfile::tempdir()?;
    let live = dir.path().join("live.sqlite");
    Connection::open(&live)?.execute_batch("CREATE TABLE items (id INTEGER, name TEXT);")?;
    let taken = dir.path().join("taken.sqlite");
    Connection::open(&taken)?.execute_batch("CREATE TABLE other (id INTEGER);")?;
    let result = sqlite_backup(&live, &taken);
    assert!(matches!(result, Err(DumpError::Sqlite(_))), "{result:?}");
    let garbage = dir.path().join("garbage.sqlite");
    std::fs::write(&garbage, "not a database, just text that is long enough")?;
    assert!(sqlite_restore(&garbage, &live).is_err());
    assert!(names(&live)?.is_empty());
    Ok(())
}
