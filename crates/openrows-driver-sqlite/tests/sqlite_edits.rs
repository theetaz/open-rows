// SPDX-License-Identifier: Apache-2.0
//! Staged edits end to end: key discovery, exact snapshots, generated
//! `SQLite` DML, atomic commits and conflict rollback (data-safety critical).
mod support;

use openrows_driver_api::Editor;
use openrows_sql::Dialect;
use openrows_sql::edit::{CellValue, Changes, Edit, EditKind, EditTable, Values, edit_sql};
use openrows_sql::select::{BoundSql, TableRef};
use support::{Collect, TestResult, code, database, run};

const SETUP: &str = "CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT, price REAL,
        doubled REAL GENERATED ALWAYS AS (price * 2) VIRTUAL);
    INSERT INTO items (id, name, price) VALUES (1, 'pen', 1.5), (2, NULL, 0.1);
    CREATE TABLE tags (code TEXT NOT NULL, label TEXT, UNIQUE (code));
    CREATE TABLE loose (a TEXT, b TEXT)";

fn table(key: Vec<String>, protected: Vec<String>) -> EditTable {
    EditTable {
        table: TableRef {
            schema: "main".into(),
            name: "items".into(),
        },
        columns: vec!["id".into(), "name".into(), "price".into(), "doubled".into()],
        key,
        protected,
    }
}

fn edit(kind: EditKind, original: Values, values: Changes) -> Edit {
    Edit {
        kind,
        original,
        values,
    }
}

async fn count(
    session: &mut openrows_driver_sqlite::SqliteSession,
    sql: &str,
) -> Result<u64, Box<dyn std::error::Error>> {
    let mut sink = Collect::default();
    session_execute(session, sql, &mut sink).await?;
    Ok(sink.rows() as u64)
}

async fn session_execute(
    session: &mut openrows_driver_sqlite::SqliteSession,
    sql: &str,
    sink: &mut Collect,
) -> Result<(), openrows_driver_api::DriverError> {
    openrows_driver_api::Session::execute(session, sql, sink)
        .await
        .map(|_| ())
}

#[tokio::test]
async fn keys_and_generated_columns_are_discovered() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut session = database(&dir, SETUP).await?;
    let items = session.row_edit_info("main", "items").await?;
    assert_eq!(
        (items.key, items.protected),
        (vec!["id".to_owned()], vec!["doubled".to_owned()])
    );
    let tags = session.row_edit_info("main", "tags").await?;
    assert_eq!(
        tags.key,
        ["code"],
        "a unique NOT NULL index identifies rows"
    );
    let loose = session.row_edit_info("main", "loose").await?;
    assert_eq!(
        (loose.key, loose.row_id),
        (vec!["rowid".to_owned()], Some("rowid".to_owned())),
        "a keyless rowid table is identified by its rowid"
    );
    Ok(())
}

#[tokio::test]
async fn snapshots_and_generated_edits_commit_atomically() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut session = database(&dir, SETUP).await?;
    let info = session.row_edit_info("main", "items").await?;
    let original: Values = session
        .edit_snapshot(("main", "items"), &[("id".into(), "2".into())])
        .await?
        .into_iter()
        .collect();
    assert_eq!(
        original.get("price").cloned().flatten().as_deref(),
        Some("0.1")
    );
    assert_eq!(original.get("name").cloned(), Some(None));
    let table = table(info.key, info.protected);
    let update = edit(
        EditKind::Update,
        original.clone(),
        Changes::from([("name".into(), CellValue::Text("cup".into()))]),
    );
    let insert = edit(
        EditKind::Insert,
        Values::new(),
        Changes::from([
            ("id".into(), CellValue::Text("3".into())),
            ("price".into(), CellValue::Text("2".into())),
        ]),
    );
    let statements: Vec<BoundSql> = [update, insert]
        .iter()
        .map(|edit| edit_sql(Dialect::Sqlite, &table, edit))
        .collect::<Result<_, _>>()?;
    session.commit_edits(&statements).await?;
    assert_eq!(
        count(
            &mut session,
            "SELECT 1 FROM items WHERE id = 2 AND name = 'cup'"
        )
        .await?,
        1
    );
    assert_eq!(
        count(
            &mut session,
            "SELECT 1 FROM items WHERE id = 3 AND typeof(price) = 'real'"
        )
        .await?,
        1
    );
    let stale = edit(EditKind::Delete, original, Changes::new());
    let stale = edit_sql(Dialect::Sqlite, &table, &stale)?;
    let fresh_snapshot: Values = session
        .edit_snapshot(("main", "items"), &[("id".into(), "1".into())])
        .await?
        .into_iter()
        .collect();
    let fresh = edit_sql(
        Dialect::Sqlite,
        &table,
        &edit(EditKind::Delete, fresh_snapshot, Changes::new()),
    )?;
    let result = session.commit_edits(&[fresh, stale]).await;
    assert_eq!(code(result), "40001", "the stale guard matches nothing");
    assert_eq!(
        count(&mut session, "SELECT 1 FROM items").await?,
        3,
        "everything rolled back"
    );
    Ok(())
}

#[tokio::test]
async fn failing_statements_roll_back_the_whole_commit() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut session = database(&dir, SETUP).await?;
    let insert = |id: &str| BoundSql {
        sql: "INSERT INTO items (id, name) VALUES (?1, 'x')".into(),
        params: vec![Some(id.into())],
    };
    let result = session.commit_edits(&[insert("10"), insert("1")]).await;
    assert_eq!(code(result), "23505");
    assert_eq!(
        count(&mut session, "SELECT 1 FROM items WHERE id = 10").await?,
        0
    );
    let missing = session
        .edit_snapshot(("main", "items"), &[("id".into(), "99".into())])
        .await;
    assert!(missing.is_err());
    run(&mut session, "SELECT 1").await?;
    Ok(())
}
