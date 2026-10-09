// SPDX-License-Identifier: Apache-2.0
//! The ER diagram of a schema: tables, columns and foreign keys in bulk.
use crate::support::{Collect, TestResult, connect, run};
use openrows_driver_api::Catalog;

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
