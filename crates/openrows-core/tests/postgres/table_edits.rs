// SPDX-License-Identifier: Apache-2.0
//! Atomic table edits, conflict detection, key eligibility and exact snapshots.
use super::{TestResult, config};
use openrows_driver_api::{ResultSink, Session, SinkFlow};
use openrows_driver_postgres::{BoundEdit, PgSession};
use openrows_sql::edit::{Edit, EditKind, EditTable, Values, edit_sql};
use openrows_sql::select::TableRef;
struct Discard;
impl ResultSink for Discard {
    fn begin_result(&mut self, _schema: arrow_schema::SchemaRef) {}
    fn push_batch(&mut self, _batch: arrow_array::RecordBatch) -> SinkFlow {
        SinkFlow::Continue
    }
}
async fn execute(session: &mut PgSession, sql: &str) -> TestResult {
    session.execute(sql, &mut Discard).await?;
    Ok(())
}
fn table() -> EditTable {
    EditTable {
        table: TableRef {
            schema: "d09".into(),
            name: "rows".into(),
        },
        columns: ["id", "name", "note", "amount", "computed"]
            .map(str::to_owned)
            .to_vec(),
        key: vec!["id".into()],
        protected: vec!["computed".into()],
    }
}
fn bound(edit: &Edit) -> TestResult<BoundEdit> {
    let sql = edit_sql(&table(), edit)?;
    Ok(BoundEdit {
        sql: sql.sql,
        params: sql.params,
    })
}
async fn snapshot(session: &mut PgSession, id: &str) -> TestResult<Values> {
    Ok(session
        .edit_snapshot(("d09", "rows"), &[("id".into(), id.into())])
        .await?
        .into_iter()
        .collect())
}
async fn setup() -> TestResult<PgSession> {
    let mut session = PgSession::connect(&config()?).await?;
    for sql in [
        "CREATE SCHEMA d09",
        "CREATE TABLE d09.rows (id int PRIMARY KEY, name text NOT NULL UNIQUE DEFERRABLE INITIALLY DEFERRED, note text, amount numeric, computed int GENERATED ALWAYS AS (id * 2) STORED)",
        "INSERT INTO d09.rows VALUES (1, 'one', NULL, 12345678901234567890.123456789), (2, 'two', '', 2)",
    ] {
        execute(&mut session, sql).await?;
    }
    Ok(session)
}
async fn assert_deferred_failure_rollback(session: &mut PgSession, before: &Values) -> TestResult {
    let result = session
        .commit_edits(&[bound(&Edit {
            kind: EditKind::Update,
            original: before.clone(),
            values: Values::from([("name".into(), Some("two".into()))]),
        })?])
        .await;
    assert!(result.is_err());
    assert_eq!(&snapshot(session, "1").await?, before);
    Ok(())
}
#[tokio::test]
#[ignore = "requires the disposable PostgreSQL test container"]
async fn transactional_edits_preserve_precision_and_roll_back_conflicts() -> TestResult {
    let mut session = setup().await?;
    let info = session.row_edit_info("d09", "rows").await?;
    assert_eq!(info.key, ["id"]);
    assert_eq!(info.protected, ["computed"]);
    let original = snapshot(&mut session, "1").await?;
    assert_eq!(
        original["amount"].as_deref(),
        Some("12345678901234567890.123456789")
    );
    let attack = "'; DROP SCHEMA d09 CASCADE; --";
    session
        .commit_edits(&[bound(&Edit {
            kind: EditKind::Update,
            original: original.clone(),
            values: Values::from([("name".into(), Some(attack.into()))]),
        })?])
        .await?;
    assert_eq!(
        snapshot(&mut session, "1").await?["name"].as_deref(),
        Some(attack)
    );
    let before = snapshot(&mut session, "1").await?;
    let stale = snapshot(&mut session, "2").await?;
    execute(
        &mut session,
        "UPDATE d09.rows SET note = 'concurrent' WHERE id = 2",
    )
    .await?;
    let result = session
        .commit_edits(&[
            bound(&Edit {
                kind: EditKind::Update,
                original: before.clone(),
                values: Values::from([("name".into(), Some("must roll back".into()))]),
            })?,
            bound(&Edit {
                kind: EditKind::Delete,
                original: stale,
                values: Values::new(),
            })?,
        ])
        .await;
    assert!(result.is_err());
    assert_eq!(snapshot(&mut session, "1").await?, before);
    let failed = session
        .commit_edits(&[
            bound(&Edit {
                kind: EditKind::Insert,
                original: Values::new(),
                values: Values::from([
                    ("id".into(), Some("3".into())),
                    ("name".into(), Some("new".into())),
                ]),
            })?,
            bound(&Edit {
                kind: EditKind::Update,
                original: before.clone(),
                values: Values::from([("name".into(), None)]),
            })?,
        ])
        .await;
    assert!(failed.is_err());
    assert!(snapshot(&mut session, "3").await.is_err());
    assert_deferred_failure_rollback(&mut session, &before).await?;
    session
        .commit_edits(&[bound(&Edit {
            kind: EditKind::Delete,
            original: before,
            values: Values::new(),
        })?])
        .await?;
    assert!(snapshot(&mut session, "1").await.is_err());
    execute(&mut session, "DROP SCHEMA d09 CASCADE").await?;
    Ok(())
}
#[tokio::test]
#[ignore = "requires the disposable PostgreSQL test container"]
async fn edit_identity_refuses_views_nullable_partial_expression_and_lossy_keys() -> TestResult {
    let mut session = PgSession::connect(&config()?).await?;
    for sql in [
        "CREATE SCHEMA d09_keys",
        "CREATE TABLE d09_keys.valid (a uuid NOT NULL, b text NOT NULL, UNIQUE (a,b))",
        "CREATE TABLE d09_keys.nullable (id int UNIQUE)",
        "CREATE TABLE d09_keys.partial (id int NOT NULL)",
        "CREATE UNIQUE INDEX p ON d09_keys.partial(id) WHERE id > 0",
        "CREATE TABLE d09_keys.lossy (id float8 PRIMARY KEY)",
        "CREATE TABLE d09_keys.expression (name text NOT NULL)",
        "CREATE UNIQUE INDEX e ON d09_keys.expression(lower(name))",
        "CREATE VIEW d09_keys.view AS SELECT * FROM d09_keys.valid",
    ] {
        execute(&mut session, sql).await?;
    }
    assert_eq!(
        session.row_edit_info("d09_keys", "valid").await?.key,
        ["a", "b"]
    );
    for name in ["nullable", "partial", "lossy", "expression", "view"] {
        assert!(
            session
                .row_edit_info("d09_keys", name)
                .await?
                .key
                .is_empty(),
            "{name}"
        );
    }
    execute(&mut session, "DROP SCHEMA d09_keys CASCADE").await?;
    Ok(())
}
