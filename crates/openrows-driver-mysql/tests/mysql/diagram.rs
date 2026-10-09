// SPDX-License-Identifier: Apache-2.0
//! The ER diagram of a database: tables, columns and foreign keys in bulk.
use openrows_driver_api::Catalog;

use super::support::{DATABASE, TestResult, run, session};

#[tokio::test]
#[ignore = "needs MySQL or MariaDB; run with `just test-mysql`"]
async fn the_diagram_reads_tables_columns_and_composite_keys() -> TestResult {
    let mut session = session().await?;
    run(
        &mut session,
        &[
            "DROP VIEW IF EXISTS erd_view",
            "DROP TABLE IF EXISTS erd_lines",
            "DROP TABLE IF EXISTS erd_orders",
            "CREATE TABLE erd_orders (id INT, region CHAR(2), note TEXT, PRIMARY KEY (id, region))",
            "CREATE TABLE erd_lines (no INT PRIMARY KEY, order_id INT NOT NULL, region CHAR(2) NOT NULL, \
             CONSTRAINT erd_lines_order FOREIGN KEY (order_id, region) REFERENCES erd_orders (id, region))",
            "CREATE VIEW erd_view AS SELECT * FROM erd_orders",
        ],
    )
    .await?;
    let diagram = Catalog::schema_diagram(&mut session, DATABASE, 10_000).await?;
    assert!(!diagram.truncated);
    assert!(diagram.tables.iter().all(|table| table.name != "erd_view"));
    let orders = diagram
        .tables
        .iter()
        .find(|table| table.name == "erd_orders");
    let columns: Vec<_> = orders
        .map(|table| {
            table
                .columns
                .iter()
                .map(|c| (c.name.as_str(), c.primary_key, c.nullable))
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(
        columns,
        [
            ("id", true, false),
            ("region", true, false),
            ("note", false, true)
        ]
    );
    let key = diagram
        .foreign_keys
        .iter()
        .find(|key| key.name == "erd_lines_order");
    assert_eq!(
        key.map(|key| (
            key.table.as_str(),
            key.columns.clone(),
            key.target.table.as_str(),
            key.target.columns.clone()
        )),
        Some((
            "erd_lines",
            vec!["order_id".to_owned(), "region".to_owned()],
            "erd_orders",
            vec!["id".to_owned(), "region".to_owned()],
        ))
    );
    let capped = Catalog::schema_diagram(&mut session, DATABASE, 1).await?;
    assert!(capped.truncated && capped.tables.len() == 1);
    Ok(())
}
