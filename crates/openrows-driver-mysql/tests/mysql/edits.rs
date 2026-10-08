// SPDX-License-Identifier: Apache-2.0
//! Staged edits end to end with generated `MySQL` DML (data-safety critical).
use openrows_driver_api::{Editor, Session};
use openrows_sql::Dialect;
use openrows_sql::edit::{CellValue, Changes, Edit, EditKind, EditTable, Values, edit_sql};
use openrows_sql::select::{BoundSql, TableRef};

use super::support::{Collect, DATABASE, TestResult, code, run, session};

async fn count(
    session: &mut openrows_driver_mysql::MySqlSession,
    sql: &str,
) -> Result<usize, Box<dyn std::error::Error>> {
    let mut sink = Collect::default();
    session.execute(sql, &mut sink).await?;
    Ok(sink.rows())
}

const ITEMS: &[&str] = &[
    "DROP TABLE IF EXISTS items",
    "CREATE TABLE items (id INT PRIMARY KEY, name VARCHAR(20), price FLOAT, \
     twice FLOAT AS (price * 2) VIRTUAL)",
    "INSERT INTO items (id, name, price) VALUES (1, 'Pen', 1.1), (2, 'cup', NULL)",
];

fn items(key: Vec<String>, protected: Vec<String>) -> EditTable {
    EditTable {
        table: TableRef {
            schema: DATABASE.into(),
            name: "items".into(),
        },
        columns: vec!["id".into(), "name".into(), "price".into(), "twice".into()],
        key,
        protected,
    }
}

fn rename(original: &Values, name: &str) -> Edit {
    Edit {
        kind: EditKind::Update,
        original: original.clone(),
        values: Changes::from([("name".into(), CellValue::Text(name.into()))]),
    }
}

#[tokio::test]
#[ignore = "needs MySQL or MariaDB; run with `just test-mysql`"]
async fn snapshots_guard_commits_and_conflicts_roll_back() -> TestResult {
    let mut session = session().await?;
    run(&mut session, ITEMS).await?;
    let info = session.row_edit_info(DATABASE, "items").await?;
    assert_eq!(info.key, ["id"]);
    assert_eq!(info.protected, ["twice"]);
    let table = items(info.key, info.protected);
    let original: Values = session
        .edit_snapshot((DATABASE, "items"), &[("id".into(), "1".into())])
        .await?
        .into_iter()
        .collect();
    let price = original.get("price").cloned().flatten();
    assert_eq!(price.as_deref(), Some("1.1"), "FLOAT keeps its text form");
    let insert = Edit {
        kind: EditKind::Insert,
        original: Values::new(),
        values: Changes::from([
            ("id".into(), CellValue::Text("3".into())),
            ("price".into(), CellValue::Default),
        ]),
    };
    let statements: Vec<BoundSql> = [rename(&original, "Pen"), insert]
        .iter()
        .map(|edit| edit_sql(Dialect::MySql, &table, edit))
        .collect::<Result<_, _>>()?;
    session.commit_edits(&statements).await?;
    let inserted = count(&mut session, "SELECT 1 FROM items WHERE id = 3").await?;
    assert_eq!(inserted, 1, "an unchanged UPDATE still matches its row");
    let lower = edit_sql(Dialect::MySql, &table, &rename(&original, "pen"))?;
    session.commit_edits(&[lower]).await?;
    let stale = Edit {
        kind: EditKind::Delete,
        original,
        values: Changes::new(),
    };
    let result = session
        .commit_edits(&[edit_sql(Dialect::MySql, &table, &stale)?])
        .await;
    assert_eq!(
        code(result),
        "40001",
        "'Pen' vs 'pen' differs byte for byte"
    );
    assert_eq!(count(&mut session, "SELECT 1 FROM items").await?, 3);
    Ok(())
}

#[tokio::test]
#[ignore = "needs MySQL or MariaDB; run with `just test-mysql`"]
async fn a_failing_statement_rolls_back_the_whole_commit() -> TestResult {
    let mut session = session().await?;
    run(
        &mut session,
        &[
            "DROP TABLE IF EXISTS pairs",
            "CREATE TABLE pairs (id INT PRIMARY KEY)",
            "INSERT INTO pairs VALUES (1)",
        ],
    )
    .await?;
    let insert = |id: &str| BoundSql {
        sql: "INSERT INTO pairs (id) VALUES (?)".into(),
        params: vec![Some(id.into())],
    };
    let result = session.commit_edits(&[insert("5"), insert("1")]).await;
    assert_eq!(code(result), "23000");
    assert_eq!(
        count(&mut session, "SELECT 1 FROM pairs WHERE id = 5").await?,
        0
    );
    Ok(())
}
