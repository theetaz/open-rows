// SPDX-License-Identifier: Apache-2.0
//! The DDL view's definitions rebuild the schema: a fixture dropped and
//! created again only from `Driver::table_ddl` and the listed trigger
//! definitions prints identically with `SHOW CREATE TABLE` (M1-17).
use openrows_driver_api::{Catalog, Driver, DriverError, Session};
use openrows_driver_mysql::{Flavour, MySqlDriver, MySqlSession};

use crate::support::{Collect, DATABASE, TestResult, mariadb, run, session, tidb};

/// Tables in dependency order, then the view.
const RELATIONS: [&str; 4] = ["ddl_customers", "ddl_orders", "ddl_notes", "ddl_totals"];

const TEARDOWN: [&str; 4] = [
    "DROP VIEW IF EXISTS ddl_totals",
    "DROP TABLE IF EXISTS ddl_notes",
    "DROP TABLE IF EXISTS ddl_orders",
    "DROP TABLE IF EXISTS ddl_customers",
];

/// What every server here accepts.
const TABLES: [&str; 2] = [
    "CREATE TABLE ddl_customers (id INT NOT NULL AUTO_INCREMENT, \
     email VARCHAR(80) COLLATE utf8mb4_bin NOT NULL, nick VARCHAR(20) DEFAULT 'it''s' \
     COMMENT 'Display name', PRIMARY KEY (id), UNIQUE KEY customers_email (email)) \
     AUTO_INCREMENT=7 COMMENT='Who buys'",
    "CREATE TABLE ddl_notes (id BIGINT NOT NULL PRIMARY KEY, `order by` TEXT, \
     body VARCHAR(100) CHARACTER SET latin1, FULLTEXT KEY notes_text (`order by`))",
];

const ORDERS: &str = "CREATE TABLE ddl_orders (id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY, \
     customer_id INT NOT NULL, price DECIMAL(10, 2) DEFAULT 1.50, \
     placed DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3), \
     total DECIMAL(12, 2) GENERATED ALWAYS AS (price * 2) STORED, \
     status ENUM('new', 'paid') NOT NULL DEFAULT 'new', \
     CONSTRAINT orders_customer FOREIGN KEY (customer_id) REFERENCES ddl_customers (id) \
     ON DELETE CASCADE, CONSTRAINT orders_price CHECK (price > 0), \
     KEY orders_placed (placed DESC, status))";

const VIEW: &str = "CREATE VIEW ddl_totals AS \
     SELECT customer_id, SUM(price) AS spent FROM ddl_orders GROUP BY customer_id";

const TRIGGER: &str = "CREATE TRIGGER ddl_orders_touch BEFORE INSERT ON ddl_orders \
     FOR EACH ROW SET NEW.price = NEW.price";

fn driver() -> MySqlDriver {
    MySqlDriver(if mariadb() {
        Flavour::MariaDb
    } else {
        Flavour::MySql
    })
}

/// The fixture for this server: `TiDB` has no triggers and refuses `FULLTEXT`,
/// generated columns in foreign-key tables' `CHECK`s and descending parts.
fn fixture() -> Vec<String> {
    if tidb() {
        return vec![
            TABLES[0].to_owned(),
            ORDERS
                .replace(" CONSTRAINT orders_price CHECK (price > 0),", "")
                .replace("placed DESC, status", "placed, status"),
            "CREATE TABLE ddl_notes (id BIGINT NOT NULL PRIMARY KEY, `order by` TEXT, \
             body VARCHAR(100) CHARACTER SET latin1)"
                .to_owned(),
            VIEW.to_owned(),
        ];
    }
    vec![
        TABLES[0].to_owned(),
        ORDERS.to_owned(),
        TABLES[1].to_owned(),
        VIEW.to_owned(),
        TRIGGER.to_owned(),
    ]
}

/// One text cell of the row `SHOW CREATE TABLE` prints.
async fn show_create(session: &mut MySqlSession, relation: &str) -> Result<String, String> {
    let sql = format!("SHOW CREATE TABLE {relation}");
    let mut sink = Collect::default();
    session
        .execute(&sql, &mut sink)
        .await
        .map_err(|error| format!("{sql}: {error}"))?;
    let batch = sink.batches.first().ok_or("no row")?;
    let column = batch.column(1).as_any();
    let text = column
        .downcast_ref::<arrow_array::StringArray>()
        .ok_or_else(|| format!("{sql}: second column is not text"))?;
    Ok(text.value(0).to_owned())
}

async fn snapshot(session: &mut MySqlSession) -> Result<Vec<String>, String> {
    let mut printed = Vec::new();
    for relation in RELATIONS {
        printed.push(show_create(session, relation).await?);
    }
    Ok(printed)
}

async fn trigger_definitions(session: &mut MySqlSession) -> Result<Vec<String>, DriverError> {
    let detail = session.table_detail(DATABASE, "ddl_orders").await?;
    Ok(detail
        .triggers
        .into_iter()
        .map(|trigger| trigger.definition)
        .collect())
}

#[tokio::test]
#[ignore = "needs MySQL or MariaDB: run `just test-mysql`"]
async fn generated_ddl_recreates_what_show_create_table_prints() -> TestResult {
    let mut session = session().await?;
    run(&mut session, &TEARDOWN).await?;
    for statement in fixture() {
        run(&mut session, &[&statement]).await?;
    }
    let original = snapshot(&mut session).await?;
    let driver = driver();
    let mut definitions = Vec::new();
    for relation in RELATIONS {
        let detail = session.table_detail(DATABASE, relation).await?;
        definitions.push(driver.table_ddl((DATABASE, relation), &detail)?);
    }
    let triggers = trigger_definitions(&mut session).await?;
    assert_eq!(triggers.len(), usize::from(!tidb()));
    run(&mut session, &TEARDOWN).await?;
    for definition in definitions.iter().chain(&triggers) {
        run(&mut session, &[definition]).await?;
    }
    let rebuilt = snapshot(&mut session).await?;
    let triggers_again = trigger_definitions(&mut session).await?;
    run(&mut session, &TEARDOWN).await?;
    for (before, after) in original.iter().zip(&rebuilt) {
        assert_eq!(before, after);
    }
    assert_eq!(triggers, triggers_again);
    assert!(original[1].contains("orders_customer"), "{}", original[1]);
    assert!(original[0].contains("AUTO_INCREMENT=7"), "{}", original[0]);
    Ok(())
}
