// SPDX-License-Identifier: Apache-2.0
//! Structure changes on real files: `ALTER TABLE` where it suffices, the
//! table rebuild otherwise, with data, keys, indexes and triggers kept.
mod support;

use openrows_driver_api::schema::{DefaultValue, NewColumn, SchemaChange, TableDetail};
use openrows_driver_api::structure::AlterTable;
use openrows_driver_api::{Catalog, Driver, Editor, Session};
use openrows_driver_sqlite::{SqliteDriver, SqliteSession};
use support::{Collect, TestResult, code, database, run};

const SETUP: &str = "PRAGMA foreign_keys = ON;
    CREATE TABLE customers (id INTEGER PRIMARY KEY, name TEXT NOT NULL);
    CREATE TABLE orders (
        id INTEGER PRIMARY KEY,
        customer INTEGER NOT NULL REFERENCES customers(id) ON DELETE CASCADE,
        total TEXT, -- stored as text for now
        note TEXT
    );
    CREATE INDEX orders_customer ON orders (customer);
    CREATE TABLE audit (n INTEGER);
    CREATE TRIGGER orders_audit AFTER INSERT ON orders BEGIN INSERT INTO audit VALUES (NEW.id); END;
    CREATE VIEW order_totals AS SELECT id, total FROM orders;
    INSERT INTO customers VALUES (1, 'ada'), (2, 'grace');
    INSERT INTO orders VALUES (10, 1, '12.5', NULL), (11, 2, '3', 'gift')";

async fn detail(
    session: &mut SqliteSession,
    table: &str,
) -> Result<TableDetail, Box<dyn std::error::Error>> {
    Ok(session.table_detail("main", table).await?)
}

async fn apply(session: &mut SqliteSession, changes: &[SchemaChange]) -> TestResult {
    let detail = detail(session, "orders").await?;
    let plan = SqliteDriver.alter_sql(
        &AlterTable {
            schema: "main",
            table: "orders",
            detail: &detail,
        },
        changes,
    )?;
    session.commit_ddl(&plan.statements).await?;
    Ok(())
}

async fn count(
    session: &mut SqliteSession,
    sql: &str,
) -> Result<usize, Box<dyn std::error::Error>> {
    let mut sink = Collect::default();
    session.execute(sql, &mut sink).await?;
    Ok(sink.rows())
}

/// Type, nullability and a rename: a rebuild, then `RENAME COLUMN`.
fn rebuild_changes() -> Vec<SchemaChange> {
    vec![
        SchemaChange::ColumnType {
            column: "total".into(),
            data_type: "REAL".into(),
        },
        SchemaChange::ColumnNullable {
            column: "note".into(),
            nullable: false,
        },
        SchemaChange::RenameColumn {
            column: "note".into(),
            name: "memo".into(),
        },
    ]
}

#[tokio::test]
async fn a_failing_rebuild_changes_nothing() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut session = database(&dir, SETUP).await?;
    apply(&mut session, &rebuild_changes())
        .await
        .err()
        .ok_or("NOT NULL over a NULL value must fail")?;
    assert_eq!(
        count(&mut session, "SELECT * FROM orders WHERE note IS NULL").await?,
        1
    );
    let names: Vec<_> = detail(&mut session, "orders")
        .await?
        .columns
        .into_iter()
        .map(|column| column.name)
        .collect();
    assert_eq!(names, ["id", "customer", "total", "note"]);
    let orphan = run(&mut session, "INSERT INTO orders VALUES (12, 99, 1, 'x')").await;
    assert_eq!(code(orphan), "23503", "foreign keys are enforced again");
    Ok(())
}

#[tokio::test]
async fn a_rebuild_keeps_rows_keys_indexes_triggers_and_views() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut session = database(&dir, SETUP).await?;
    run(
        &mut session,
        "UPDATE orders SET note = '' WHERE note IS NULL",
    )
    .await?;
    apply(&mut session, &rebuild_changes()).await?;
    let after = detail(&mut session, "orders").await?;
    let columns: Vec<_> = after
        .columns
        .iter()
        .map(|column| {
            (
                column.name.as_str(),
                column.data_type.as_str(),
                column.nullable,
            )
        })
        .collect();
    assert_eq!(
        columns,
        [
            ("id", "INTEGER", false),
            ("customer", "INTEGER", false),
            ("total", "REAL", true),
            ("memo", "TEXT", false)
        ]
    );
    assert!(
        after
            .indexes
            .iter()
            .any(|index| index.name == "orders_customer")
    );
    assert_eq!(after.triggers.len(), 1);
    assert_eq!(
        count(
            &mut session,
            "SELECT * FROM order_totals WHERE total = 12.5"
        )
        .await?,
        1
    );
    // Foreign keys still cascade and are enforced again.
    run(&mut session, "DELETE FROM customers WHERE id = 1").await?;
    assert_eq!(count(&mut session, "SELECT * FROM orders").await?, 1);
    let orphan = run(&mut session, "INSERT INTO orders VALUES (12, 99, 1, 'x')").await;
    assert_eq!(code(orphan), "23503");
    run(&mut session, "INSERT INTO orders VALUES (12, 2, 1, 'x')").await?;
    // Two rows from the setup, one through the recreated trigger.
    assert_eq!(count(&mut session, "SELECT * FROM audit").await?, 3);
    Ok(())
}

#[tokio::test]
async fn plain_changes_use_alter_table_and_new_indexes() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut session = database(&dir, SETUP).await?;
    apply(
        &mut session,
        &[
            SchemaChange::DropColumn("note".into()),
            SchemaChange::AddColumn(NewColumn {
                name: "qty".into(),
                data_type: "INTEGER".into(),
                nullable: false,
                default: DefaultValue::Literal("1".into()),
                comment: None,
                ..NewColumn::default()
            }),
            SchemaChange::CreateIndex(openrows_driver_api::schema::NewIndex {
                name: Some("orders_qty".into()),
                columns: vec!["qty".into()],
                unique: false,
                method: openrows_driver_api::schema::IndexMethod::Btree,
                ..openrows_driver_api::schema::NewIndex::default()
            }),
            SchemaChange::RenameTable("purchases".into()),
        ],
    )
    .await?;
    let after = detail(&mut session, "purchases").await?;
    let names: Vec<_> = after
        .columns
        .iter()
        .map(|column| column.name.as_str())
        .collect();
    assert_eq!(names, ["id", "customer", "total", "qty"]);
    assert!(after.indexes.iter().any(|index| index.name == "orders_qty"));
    assert_eq!(
        count(&mut session, "SELECT * FROM purchases WHERE qty = 1").await?,
        2
    );
    Ok(())
}
