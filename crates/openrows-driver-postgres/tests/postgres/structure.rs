// SPDX-License-Identifier: Apache-2.0
//! Table detail, rebuilt DDL and transactional structure changes against a
//! real server.
use crate::support::{Collect, TestResult, connect, run};
use openrows_driver_api::schema::{
    ConstraintKind, DefaultValue, Generated, IndexMethod, NewColumn, NewIndex, SchemaChange,
    TableDetail, TriggerTiming,
};
use openrows_driver_postgres::PgSession;
use openrows_driver_postgres::ddl::{AlterTable, alter_sql, create_table_sql};

/// One statement each: the driver executes through the extended protocol.
const FIXTURE: [&str; 11] = [
    "DROP SCHEMA IF EXISTS t02 CASCADE",
    "CREATE SCHEMA t02",
    "CREATE TABLE t02.customers (id int PRIMARY KEY)",
    "CREATE TABLE t02.orders (id int GENERATED ALWAYS AS IDENTITY PRIMARY KEY, \
     \"Note\" text DEFAULT 'n/a', price numeric CHECK (price > 0), \
     customer_id int REFERENCES t02.customers(id), total numeric GENERATED ALWAYS AS (price * 2) STORED)",
    "CREATE INDEX orders_note_idx ON t02.orders (lower(\"Note\")) INCLUDE (price) WHERE price > 1",
    "COMMENT ON TABLE t02.orders IS 'Customer''s orders'",
    "COMMENT ON COLUMN t02.orders.price IS 'Gross'",
    "CREATE FUNCTION t02.touch() RETURNS trigger LANGUAGE plpgsql AS 'BEGIN RETURN NEW; END'",
    "CREATE TRIGGER orders_touch BEFORE INSERT OR UPDATE ON t02.orders FOR EACH ROW EXECUTE FUNCTION t02.touch()",
    "INSERT INTO t02.customers VALUES (1)",
    "INSERT INTO t02.orders (\"Note\", price, customer_id) VALUES ('a', 3, 1)",
];

async fn fixture(session: &mut PgSession) -> TestResult {
    for statement in FIXTURE {
        let (created, _) = run(session, statement, Collect::default()).await;
        created?;
    }
    Ok(())
}

#[tokio::test]
#[ignore = "needs PostgreSQL: run `just test-pg`"]
async fn detail_lists_columns_constraints_indexes_triggers_and_sizes() -> TestResult {
    let mut session = connect().await?;
    fixture(&mut session).await?;
    let detail = session.table_detail("t02", "orders").await?;
    let names: Vec<&str> = detail.columns.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["id", "Note", "price", "customer_id", "total"]);
    assert_eq!(detail.columns[0].generated, Generated::IdentityAlways);
    assert_eq!(detail.columns[4].generated, Generated::Stored);
    assert_eq!(detail.columns[2].comment.as_deref(), Some("Gross"));
    let foreign = detail
        .constraints
        .iter()
        .find(|c| c.kind == ConstraintKind::Foreign)
        .and_then(|c| c.references.clone())
        .ok_or("foreign key")?;
    assert_eq!(
        (foreign.table.as_str(), foreign.columns.as_slice()),
        ("customers", &["id".to_owned()][..])
    );
    let index = detail
        .indexes
        .iter()
        .find(|i| i.name == "orders_note_idx")
        .ok_or("index")?;
    assert_eq!(index.include, vec!["price"]);
    assert!(!index.constraint && index.predicate.is_some());
    assert!(detail.indexes.iter().any(|i| i.primary && i.constraint));
    assert_eq!(detail.triggers[0].timing, TriggerTiming::Before);
    let info = detail.info.clone().ok_or("info")?;
    assert!(info.total_bytes >= info.data_bytes + info.index_bytes);
    assert_eq!(info.comment.as_deref(), Some("Customer's orders"));
    let ddl = create_table_sql("t02", "orders", &detail)?;
    assert!(ddl.contains("CREATE TABLE \"t02\".\"orders\""), "{ddl}");
    assert!(ddl.contains("GENERATED ALWAYS AS IDENTITY"), "{ddl}");
    assert!(ddl.contains("CREATE INDEX orders_note_idx"), "{ddl}");
    assert!(ddl.contains("IS 'Customer''s orders';"), "{ddl}");
    assert!(session.table_detail("t02", "missing").await?.info.is_none());
    Ok(())
}

#[tokio::test]
#[ignore = "needs PostgreSQL: run `just test-pg`"]
async fn change_sets_commit_atomically_and_roll_back_on_error() -> TestResult {
    let mut session = connect().await?;
    fixture(&mut session).await?;
    let changes = [
        SchemaChange::RenameColumn {
            column: "Note".into(),
            name: "memo".into(),
        },
        SchemaChange::ColumnDefault {
            column: "Note".into(),
            default: DefaultValue::Literal("it's".into()),
        },
        SchemaChange::ColumnType {
            column: "price".into(),
            data_type: "numeric(12, 2)".into(),
        },
        SchemaChange::AddColumn(NewColumn {
            name: "at".into(),
            data_type: "timestamptz".into(),
            nullable: false,
            default: DefaultValue::Expression("now()".into()),
            comment: Some("Created".into()),
        }),
        SchemaChange::CreateIndex(NewIndex {
            name: Some("orders_at_idx".into()),
            columns: vec!["at".into(), "memo".into()],
            unique: false,
            method: IndexMethod::Btree,
        }),
        SchemaChange::DropIndex("orders_note_idx".into()),
        SchemaChange::TableComment(None),
        SchemaChange::RenameTable("purchases".into()),
    ];
    let detail = session.table_detail("t02", "orders").await?;
    let target = AlterTable {
        schema: "t02",
        table: "orders",
        detail: &detail,
    };
    session
        .commit_ddl(&alter_sql(&target, &changes)?.statements)
        .await?;
    let after = session.table_detail("t02", "purchases").await?;
    let memo = after
        .columns
        .iter()
        .find(|c| c.name == "memo")
        .ok_or("memo")?;
    assert_eq!(memo.default.as_deref(), Some("'it''s'::text"));
    assert!(after.columns.iter().any(|c| c.name == "at" && !c.nullable));
    assert!(after.indexes.iter().any(|i| i.name == "orders_at_idx"));
    assert!(
        after
            .info
            .as_ref()
            .is_some_and(|info| info.comment.is_none())
    );
    rolls_back_on_error(&mut session, &after).await
}

/// A failing statement (text that cannot be cast) rolls back every change.
async fn rolls_back_on_error(session: &mut PgSession, after: &TableDetail) -> TestResult {
    let failing = [
        SchemaChange::RenameColumn {
            column: "memo".into(),
            name: "kept".into(),
        },
        SchemaChange::ColumnType {
            column: "memo".into(),
            data_type: "integer".into(),
        },
    ];
    let target = AlterTable {
        schema: "t02",
        table: "purchases",
        detail: after,
    };
    let error = session
        .commit_ddl(&alter_sql(&target, &failing)?.statements)
        .await;
    assert!(error.is_err());
    let unchanged = session.table_detail("t02", "purchases").await?;
    assert!(
        unchanged
            .columns
            .iter()
            .any(|c| c.name == "memo" && c.data_type == "text")
    );
    Ok(())
}
