// SPDX-License-Identifier: Apache-2.0
//! Keyless rowid tables (docs/10 §4.3.3, data-safety critical): rows are
//! identified by the rowid, every edit changes exactly the targeted row, a
//! stale row rolls the whole commit back, and tables without a usable rowid
//! stay read-only.
mod support;

use arrow_schema::DataType;
use openrows_driver_api::{Catalog, Editor};
use openrows_driver_sqlite::SqliteSession;
use openrows_sql::Dialect;
use openrows_sql::edit::{CellValue, Changes, Edit, EditKind, EditTable, Values, edit_sql};
use openrows_sql::select::{Combinator, Page, SelectError, TableQuery, TableRef, page_sql};
use openrows_sql::{StatementKind, classify};
use support::{Collect, TestResult, code, database, run};

const SETUP: &str = "CREATE TABLE loose (a TEXT, b INTEGER);
    INSERT INTO loose VALUES ('x', 1), ('same', 2), ('same', 2), ('z', NULL);
    CREATE TABLE shadow (rowid TEXT, a TEXT);
    INSERT INTO shadow VALUES ('r', 'one'), ('r', 'one');
    CREATE TABLE covered (RowId TEXT, _ROWID_ TEXT, Oid TEXT);
    CREATE TABLE strict_key (k TEXT PRIMARY KEY, v TEXT) WITHOUT ROWID;
    CREATE TABLE counted (id INTEGER PRIMARY KEY AUTOINCREMENT, v TEXT);
    INSERT INTO counted (v) VALUES ('a');
    CREATE VIEW loose_view AS SELECT * FROM loose";

type Failure = Box<dyn std::error::Error>;

/// The app's edit shape: catalog columns plus the hidden rowid.
async fn shape(session: &mut SqliteSession, name: &str) -> Result<EditTable, Failure> {
    let info = session.row_edit_info("main", name).await?;
    let mut columns: Vec<String> = session
        .table_columns("main", name)
        .await?
        .into_iter()
        .map(|column| column.name)
        .collect();
    columns.extend(info.row_id.clone());
    Ok(EditTable {
        table: TableRef {
            schema: "main".into(),
            name: name.into(),
        },
        columns,
        key: info.key,
        protected: info.protected,
    })
}

async fn snapshot(
    session: &mut SqliteSession,
    table: &EditTable,
    id: &str,
) -> Result<Values, Failure> {
    let key = table.key.first().cloned().unwrap_or_default();
    let target = (table.table.schema.as_str(), table.table.name.as_str());
    Ok(session
        .edit_snapshot(target, &[(key, id.to_owned())])
        .await?
        .into_iter()
        .collect())
}

fn edit(kind: EditKind, original: Values, values: Changes) -> Edit {
    Edit {
        kind,
        original,
        values,
    }
}

fn set(name: &str, value: &str) -> Changes {
    Changes::from([(name.to_owned(), CellValue::Text(value.to_owned()))])
}

async fn count(session: &mut SqliteSession, sql: &str) -> Result<usize, Failure> {
    let mut sink = Collect::default();
    openrows_driver_api::Session::execute(session, sql, &mut sink).await?;
    Ok(sink.rows())
}

#[tokio::test]
async fn a_keyless_rowid_table_is_keyed_by_its_rowid() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut session = database(&dir, SETUP).await?;
    let info = session.row_edit_info("main", "loose").await?;
    assert_eq!(info.key, ["rowid"]);
    assert_eq!(info.protected, ["rowid"], "the rowid is never written");
    assert_eq!(info.row_id.as_deref(), Some("rowid"));
    let keyed = session.row_edit_info("main", "counted").await?;
    assert_eq!((keyed.key, keyed.row_id), (vec!["id".to_owned()], None));
    Ok(())
}

#[tokio::test]
async fn shadowed_aliases_fall_back_and_tables_without_a_rowid_stay_read_only() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut session = database(&dir, SETUP).await?;
    let shadow = session.row_edit_info("main", "shadow").await?;
    assert_eq!(shadow.key, ["_rowid_"], "a real `rowid` column is data");
    assert_eq!(shadow.row_id.as_deref(), Some("_rowid_"));
    for name in ["covered", "loose_view", "sqlite_sequence", "missing"] {
        let info = session.row_edit_info("main", name).await?;
        assert!(info.key.is_empty() && info.row_id.is_none(), "{name}");
    }
    let strict = session.row_edit_info("main", "strict_key").await?;
    assert_eq!(strict.key, ["k"]);
    assert_eq!(strict.row_id, None, "WITHOUT ROWID never uses a rowid");
    Ok(())
}

#[tokio::test]
async fn the_browse_selects_the_integer_rowid_last() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut session = database(&dir, SETUP).await?;
    let table = shape(&mut session, "shadow").await?;
    let known: Vec<String> = table.columns[..2].to_vec();
    let query = TableQuery {
        table: table.table.clone(),
        columns: Vec::new(),
        filters: Vec::new(),
        groups: Vec::new(),
        combinator: Combinator::And,
        sort: None,
        row_id: Some("_rowid_".into()),
    };
    let page = Page {
        limit: 10,
        offset: 0,
    };
    let bound = page_sql(Dialect::Sqlite, &query, &known, page)?;
    assert_eq!(
        classify(&bound.sql, Dialect::Sqlite).kind,
        StatementKind::Read
    );
    let mut sink = Collect::default();
    openrows_driver_api::Session::execute(&mut session, &bound.sql, &mut sink).await?;
    let fields: Vec<(String, DataType)> = sink.schemas[0]
        .fields()
        .iter()
        .map(|field| (field.name().clone(), field.data_type().clone()))
        .collect();
    assert_eq!(
        fields.last(),
        Some(&("_rowid_".to_owned(), DataType::Int64))
    );
    assert_eq!(fields[0].0, "rowid", "the real column keeps its name");
    let rowid = Some("rowid".to_owned());
    let shadowed = TableQuery {
        row_id: rowid,
        ..query
    };
    assert_eq!(
        page_sql(Dialect::Sqlite, &shadowed, &known, page),
        Err(SelectError::UnknownColumn("rowid".into()))
    );
    Ok(())
}

#[tokio::test]
async fn updates_and_deletes_by_rowid_change_exactly_one_row() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut session = database(&dir, SETUP).await?;
    let table = shape(&mut session, "loose").await?;
    let first = snapshot(&mut session, &table, "1").await?;
    assert_eq!(first.get("rowid").cloned().flatten().as_deref(), Some("1"));
    assert_eq!(first.get("a").cloned().flatten().as_deref(), Some("x"));
    let last = snapshot(&mut session, &table, "4").await?;
    assert_eq!(last.get("b").cloned(), Some(None));
    let update = edit(EditKind::Update, first, set("a", "changed"));
    let delete = edit(EditKind::Delete, last, Changes::new());
    let statements = [update, delete]
        .iter()
        .map(|edit| edit_sql(Dialect::Sqlite, &table, edit))
        .collect::<Result<Vec<_>, _>>()?;
    assert!(statements.iter().all(|bound| {
        let class = classify(&bound.sql, Dialect::Sqlite);
        class.kind == StatementKind::Write && !class.dangerous
    }));
    session.commit_edits(&statements).await?;
    assert_eq!(
        count(
            &mut session,
            "SELECT 1 FROM loose WHERE rowid = 1 AND a = 'changed'"
        )
        .await?,
        1
    );
    assert_eq!(
        count(&mut session, "SELECT 1 FROM loose WHERE rowid = 4").await?,
        0
    );
    assert_eq!(count(&mut session, "SELECT 1 FROM loose").await?, 3);
    Ok(())
}

#[tokio::test]
async fn identical_rows_are_edited_one_at_a_time() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut session = database(&dir, SETUP).await?;
    let table = shape(&mut session, "loose").await?;
    let original = snapshot(&mut session, &table, "3").await?;
    let update = edit_sql(
        Dialect::Sqlite,
        &table,
        &edit(EditKind::Update, original, set("b", "9")),
    )?;
    session.commit_edits(&[update]).await?;
    assert_eq!(
        count(
            &mut session,
            "SELECT 1 FROM loose WHERE a = 'same' AND b = 2 AND rowid = 2"
        )
        .await?,
        1
    );
    assert_eq!(
        count(
            &mut session,
            "SELECT 1 FROM loose WHERE b = 9 AND rowid = 3"
        )
        .await?,
        1
    );
    let shadow = shape(&mut session, "shadow").await?;
    let twin = snapshot(&mut session, &shadow, "2").await?;
    assert_eq!(twin.get("rowid").cloned().flatten().as_deref(), Some("r"));
    let delete = edit_sql(
        Dialect::Sqlite,
        &shadow,
        &edit(EditKind::Delete, twin, Changes::new()),
    )?;
    session.commit_edits(&[delete]).await?;
    assert_eq!(
        count(
            &mut session,
            "SELECT 1 FROM shadow WHERE _rowid_ = 1 AND rowid = 'r'"
        )
        .await?,
        1
    );
    assert_eq!(count(&mut session, "SELECT 1 FROM shadow").await?, 1);
    Ok(())
}

#[tokio::test]
async fn a_row_changed_after_its_snapshot_rolls_everything_back() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut session = database(&dir, SETUP).await?;
    let table = shape(&mut session, "loose").await?;
    let fresh = snapshot(&mut session, &table, "1").await?;
    let stale = snapshot(&mut session, &table, "2").await?;
    run(&mut session, "UPDATE loose SET b = 5 WHERE rowid = 2").await?;
    let statements = [
        edit(EditKind::Update, fresh, set("a", "lost")),
        edit(EditKind::Update, stale, set("a", "lost")),
    ]
    .iter()
    .map(|edit| edit_sql(Dialect::Sqlite, &table, edit))
    .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(code(session.commit_edits(&statements).await), "40001");
    assert_eq!(
        count(&mut session, "SELECT 1 FROM loose WHERE a = 'lost'").await?,
        0
    );
    let moved = snapshot(&mut session, &table, "3").await?;
    run(&mut session, "DELETE FROM loose WHERE rowid = 3").await?;
    let delete = edit_sql(
        Dialect::Sqlite,
        &table,
        &edit(EditKind::Delete, moved, Changes::new()),
    )?;
    assert_eq!(code(session.commit_edits(&[delete]).await), "40001");
    assert!(
        snapshot(&mut session, &table, "3").await.is_err(),
        "a vanished row"
    );
    Ok(())
}

#[tokio::test]
async fn the_rowid_is_refused_as_a_value_before_sql_exists() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut session = database(&dir, SETUP).await?;
    let table = shape(&mut session, "loose").await?;
    let original = snapshot(&mut session, &table, "1").await?;
    let rewrite = edit(EditKind::Update, original, set("rowid", "99"));
    assert!(edit_sql(Dialect::Sqlite, &table, &rewrite).is_err());
    let insert = edit(EditKind::Insert, Values::new(), set("rowid", "99"));
    assert!(edit_sql(Dialect::Sqlite, &table, &insert).is_err());
    let plain = edit(EditKind::Insert, Values::new(), set("a", "new"));
    let bound = edit_sql(Dialect::Sqlite, &table, &plain)?;
    assert!(!bound.sql.contains("rowid"), "inserts never mention it");
    session.commit_edits(&[bound]).await?;
    assert_eq!(
        count(&mut session, "SELECT 1 FROM loose WHERE a = 'new'").await?,
        1
    );
    let strict = shape(&mut session, "strict_key").await?;
    let by_rowid = [("rowid".to_owned(), "1".to_owned())];
    let keyed = session
        .edit_snapshot(("main", "strict_key"), &by_rowid)
        .await;
    assert!(keyed.is_err(), "WITHOUT ROWID has no rowid to read by");
    assert_eq!(strict.columns, ["k", "v"]);
    Ok(())
}
