// SPDX-License-Identifier: Apache-2.0
//! The ER diagram of a schema: tables, columns and foreign keys in bulk.
use crate::support::{Collect, TestResult, connect, run};
use arrow_array::cast::AsArray as _;
use openrows_driver_api::{Catalog, Session as _};

const FIXTURE: [&str; 7] = [
    "DROP SCHEMA IF EXISTS erd CASCADE",
    "CREATE SCHEMA erd",
    "CREATE TABLE erd.orders (id int, region text, note text, PRIMARY KEY (id, region))",
    "CREATE TABLE erd.lines (no int PRIMARY KEY, order_id int NOT NULL, region text NOT NULL, \
     parent int REFERENCES erd.lines (no), \
     CONSTRAINT lines_order FOREIGN KEY (order_id, region) REFERENCES erd.orders (id, region))",
    "CREATE TABLE erd.nothing ()",
    "CREATE VIEW erd.recent AS SELECT * FROM erd.orders",
    "CREATE TABLE erd.events (day date) PARTITION BY RANGE (day)",
];

#[tokio::test]
#[ignore = "needs PostgreSQL: run `just test-pg`"]
async fn the_diagram_reads_tables_columns_and_composite_keys() -> TestResult {
    let mut session = connect().await?;
    for statement in FIXTURE {
        let (created, _) = run(&mut session, statement, Collect::default()).await;
        created?;
    }
    let (partition, _) = run(
        &mut session,
        "CREATE TABLE erd.events_2026 PARTITION OF erd.events FOR VALUES FROM ('2026-01-01') TO ('2027-01-01')",
        Collect::default(),
    )
    .await;
    partition?;
    let diagram = Catalog::schema_diagram(&mut session, "erd", 100).await?;
    let names: Vec<_> = diagram.tables.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["events", "lines", "nothing", "orders"]);
    assert!(!diagram.truncated);
    let orders = &diagram.tables[3];
    let columns: Vec<_> = orders
        .columns
        .iter()
        .map(|c| {
            (
                c.name.as_str(),
                c.data_type.as_str(),
                c.primary_key,
                c.nullable,
            )
        })
        .collect();
    assert_eq!(
        columns,
        [
            ("id", "integer", true, false),
            ("region", "text", true, false),
            ("note", "text", false, true)
        ]
    );
    let keys: Vec<_> = diagram
        .foreign_keys
        .iter()
        .map(|k| {
            (
                k.table.as_str(),
                k.columns.join(","),
                k.target.table.as_str(),
                k.target.columns.join(","),
            )
        })
        .collect();
    assert_eq!(
        keys,
        [
            (
                "lines",
                "order_id,region".to_owned(),
                "orders",
                "id,region".to_owned()
            ),
            ("lines", "parent".to_owned(), "lines", "no".to_owned()),
        ]
    );
    let capped = Catalog::schema_diagram(&mut session, "erd", 2).await?;
    assert!(capped.truncated);
    assert_eq!(capped.tables.len(), 2);
    assert!(
        capped
            .foreign_keys
            .iter()
            .all(|k| k.target.table == "lines")
    );
    Ok(())
}

#[tokio::test]
#[ignore = "needs PostgreSQL: run `just test-pg`"]
async fn table_detail_carries_cross_schema_keys_and_the_jump_selects_the_referenced_row()
-> TestResult {
    let mut session = connect().await?;
    for statement in [
        "CREATE SCHEMA fu03_a",
        "CREATE SCHEMA fu03_b",
        "CREATE TABLE fu03_b.customers (id int PRIMARY KEY, name text)",
        "CREATE TABLE fu03_a.orders (id int PRIMARY KEY, customer_id int REFERENCES fu03_b.customers(id))",
        "INSERT INTO fu03_b.customers VALUES (7, 'selected'), (8, 'other')",
        "INSERT INTO fu03_a.orders VALUES (1, 7)",
    ] {
        let (reply, _) = run(&mut session, statement, Collect::default()).await;
        reply?;
    }
    let detail = Catalog::table_detail(&mut session, "fu03_a", "orders").await?;
    let constraint = detail
        .constraints
        .iter()
        .find(|key| key.references.is_some())
        .ok_or("missing cross-schema key")?;
    let target = constraint.references.as_ref().ok_or("missing target")?;
    assert_eq!(
        (target.schema.as_str(), target.table.as_str()),
        ("fu03_b", "customers")
    );
    assert_eq!(constraint.columns, ["customer_id"]);
    assert_eq!(target.columns, ["id"]);
    let query = openrows_sql::select::TableQuery {
        table: openrows_sql::select::TableRef {
            schema: target.schema.clone(),
            name: target.table.clone(),
        },
        columns: Vec::new(),
        filters: vec![openrows_sql::select::Filter {
            column: target.columns[0].clone(),
            op: openrows_sql::select::FilterOp::Equal,
            value: "7".into(),
            second: String::new(),
        }],
        groups: Vec::new(),
        combinator: openrows_sql::select::Combinator::And,
        sort: None,
        row_id: None,
    };
    let sql = openrows_sql::select::page_sql(
        openrows_sql::Dialect::Postgres,
        &query,
        &["id".into(), "name".into()],
        openrows_sql::select::Page {
            limit: 10,
            offset: 0,
        },
    )?;
    assert!(sql.sql.contains("\"fu03_b\".\"customers\""));
    assert_eq!(sql.params, [Some("7".into())]);
    let mut sink = Collect::default();
    let result = session
        .execute_bound(&sql.sql, &sql.params, &mut sink)
        .await?;
    assert_eq!(result.rows, 1);
    let batch = sink.batches.first().ok_or("missing referenced row")?;
    assert_eq!(batch.column(1).as_string::<i32>().value(0), "selected");
    for statement in ["DROP SCHEMA fu03_a CASCADE", "DROP SCHEMA fu03_b CASCADE"] {
        let (reply, _) = run(&mut session, statement, Collect::default()).await;
        reply?;
    }
    Ok(())
}
