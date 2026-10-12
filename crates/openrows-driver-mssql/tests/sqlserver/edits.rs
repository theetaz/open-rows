// SPDX-License-Identifier: Apache-2.0
//! Keys, exact snapshots and guarded commits on types whose default text
//! loses precision, with a trigger that writes rows of its own.
use arrow_array::cast::AsArray;
use openrows_driver_api::{DriverError, Editor, Session};
use openrows_sql::Dialect;
use openrows_sql::edit::{CellValue, Edit, EditKind, EditTable, edit_sql};
use openrows_sql::select::{BoundSql, TableRef};

use super::support::{Collect, TestResult, code, fixture, run, session};

const SCHEMA: &str = "dbo";

fn table(name: &str, key: Vec<String>, protected: Vec<String>) -> EditTable {
    EditTable {
        table: TableRef {
            schema: SCHEMA.to_owned(),
            name: name.to_owned(),
        },
        columns: ["id", "f", "m", "d", "g", "b", "note", "seq"]
            .map(str::to_owned)
            .to_vec(),
        key,
        protected,
    }
}

fn generate(
    table: &EditTable,
    edits: &[Edit],
) -> Result<Vec<BoundSql>, Box<dyn std::error::Error>> {
    Ok(edits
        .iter()
        .map(|edit| edit_sql(Dialect::SqlServer, table, edit))
        .collect::<Result<_, _>>()?)
}

#[tokio::test]
#[ignore = "needs SQL Server; run with `just test-mssql`"]
async fn exact_snapshots_guard_commits_through_triggers() -> TestResult {
    let mut session = session().await?;
    let (name, audit) = (fixture("edits"), fixture("audit"));
    run(
        &mut session,
        &[
            &format!(
                "CREATE TABLE dbo.{name} (id int PRIMARY KEY, f float, m money, d datetime, \
                 g uniqueidentifier, b varbinary(8), note nvarchar(20), seq int IDENTITY)"
            ),
            &format!("CREATE TABLE dbo.{audit} (n int)"),
            &format!(
                "CREATE TRIGGER dbo.{name}_tr ON dbo.{name} AFTER UPDATE AS \
                 INSERT INTO dbo.{audit} VALUES (1), (2)"
            ),
            &format!(
                "INSERT INTO dbo.{name} (id, f, m, d, g, b, note) VALUES (1, 3.141592653589793, \
                 12.3456, '2024-01-02 03:04:05.007', '00112233-4455-6677-8899-AABBCCDDEEFF', \
                 0x00FF, N'a  ')"
            ),
        ],
    )
    .await?;
    let outcome = check(&mut session, &name).await;
    run(
        &mut session,
        &[
            &format!("DROP TABLE dbo.{name}"),
            &format!("DROP TABLE dbo.{audit}"),
        ],
    )
    .await?;
    outcome
}

async fn check(session: &mut openrows_driver_mssql::SqlServerSession, name: &str) -> TestResult {
    let info = session.row_edit_info(SCHEMA, name).await?;
    assert_eq!(info.key, ["id"]);
    assert_eq!(info.protected, ["seq"]);
    let snapshot: std::collections::BTreeMap<_, _> = session
        .edit_snapshot((SCHEMA, name), &[("id".to_owned(), "1".to_owned())])
        .await?
        .into_iter()
        .collect();
    let value = |column: &str| snapshot.get(column).cloned().flatten().unwrap_or_default();
    let floating = value("f");
    assert_eq!(floating, "3.1415926535897931e+000");
    assert_eq!(
        floating.parse::<f64>()?.to_bits(),
        std::f64::consts::PI.to_bits()
    );
    assert_eq!(value("m"), "12.3456");
    assert_eq!(value("d"), "2024-01-02 03:04:05.007");
    assert_eq!(value("g"), "00112233-4455-6677-8899-aabbccddeeff");
    assert_eq!(value("b"), "0x00FF");
    assert_eq!(value("note"), "a  ");
    let table = table(name, info.key, info.protected);
    let update = |original, note: &str| Edit {
        kind: EditKind::Update,
        original,
        values: [("note".to_owned(), CellValue::Text(note.to_owned()))].into(),
    };
    session
        .commit_edits(&generate(&table, &[update(snapshot.clone(), "b")])?)
        .await?;
    // The stored note no longer matches (`a  ` vs `b`); trailing spaces count.
    let stale = session
        .commit_edits(&generate(&table, &[update(snapshot, "c")])?)
        .await;
    assert_eq!(code(stale), "40001");
    let mut sink = Collect::default();
    session
        .execute(&format!("SELECT note FROM dbo.{name}"), &mut sink)
        .await?;
    assert_eq!(sink.batches[0].column(0).as_string::<i32>().value(0), "b");
    assert!(!session.in_transaction());
    changed_float_rejects_snapshot(session, name, &table).await
}

async fn changed_float_rejects_snapshot(
    session: &mut openrows_driver_mssql::SqlServerSession,
    name: &str,
    table: &EditTable,
) -> TestResult {
    let original = session
        .edit_snapshot((SCHEMA, name), &[("id".to_owned(), "1".to_owned())])
        .await?
        .into_iter()
        .collect();
    run(
        session,
        &[&format!(
            "UPDATE dbo.{name} SET f = 3.1415926535897936 WHERE id = 1"
        )],
    )
    .await?;
    let update = Edit {
        kind: EditKind::Update,
        original,
        values: [("note".to_owned(), CellValue::Text("stale float".into()))].into(),
    };
    let stale = session.commit_edits(&generate(table, &[update])?).await;
    assert_eq!(code(stale), "40001");
    let mut sink = Collect::default();
    session
        .execute(&format!("SELECT note FROM dbo.{name}"), &mut sink)
        .await?;
    assert_eq!(sink.batches[0].column(0).as_string::<i32>().value(0), "b");
    Ok(())
}

#[tokio::test]
#[ignore = "needs SQL Server; run with `just test-mssql`"]
async fn a_failing_ddl_set_leaves_nothing_behind() -> TestResult {
    let mut session = session().await?;
    let name = fixture("ddl");
    let statements = [
        format!("CREATE TABLE dbo.{name} (id int)"),
        "CREATE TABLE dbo.no_such_schema_x.t (id int)".to_owned(),
    ];
    let failed = session.commit_ddl(&statements).await;
    assert!(
        matches!(failed, Err(DriverError::Server { .. })),
        "{:?}",
        failed.err()
    );
    let probe = session
        .execute(
            &format!("SELECT * FROM dbo.{name}"),
            &mut Collect::default(),
        )
        .await;
    assert_eq!(code(probe), "42S02");
    assert!(!session.in_transaction());
    Ok(())
}
