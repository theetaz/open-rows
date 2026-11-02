// SPDX-License-Identifier: Apache-2.0
//! The DDL view's text rebuilds the schema: a new file created only from
//! `Driver::table_ddl` holds the same `sqlite_master` rows (M1-17).
mod support;

use arrow_array::cast::AsArray;
use arrow_array::{Array, RecordBatch};
use openrows_driver_api::{Catalog, Driver, Session};
use openrows_driver_sqlite::{SqliteDriver, SqliteSession, open};
use support::{Collect, TestResult, database, run};

const FIXTURE: &str = "
    CREATE TABLE customers (
        id INTEGER PRIMARY KEY,
        email TEXT NOT NULL UNIQUE COLLATE NOCASE, -- kept verbatim
        nick TEXT DEFAULT 'it''s'
    );
    CREATE TABLE \"order items\" (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        customer INTEGER NOT NULL REFERENCES customers(id) ON DELETE CASCADE,
        price NUMERIC CONSTRAINT positive CHECK (price > 0),
        total NUMERIC GENERATED ALWAYS AS (price * 2) STORED
    );
    CREATE TABLE pairs (a TEXT, b TEXT, PRIMARY KEY (a, b)) WITHOUT ROWID;
    CREATE TABLE audit (n INTEGER);
    CREATE INDEX items_price ON \"order items\" (price DESC) WHERE price > 1;
    CREATE UNIQUE INDEX customers_nick ON customers (lower(nick));
    CREATE TRIGGER items_audit AFTER INSERT ON \"order items\"
        BEGIN INSERT INTO audit VALUES (NEW.id); END;
    CREATE VIEW item_totals AS SELECT customer, SUM(price) AS spent FROM \"order items\" GROUP BY customer";

/// Relations in dependency order, as the DDL view lists them one at a time.
const RELATIONS: [&str; 5] = ["customers", "audit", "order items", "pairs", "item_totals"];

/// `sqlite_sequence` appears with the first `AUTOINCREMENT` table; its row is not DDL.
const ROWS: &str = "SELECT type, name, tbl_name, COALESCE(sql, '') FROM sqlite_master \
    WHERE name <> 'sqlite_sequence' ORDER BY type, name";

fn cells(batch: &RecordBatch) -> Vec<String> {
    let columns: Vec<_> = (0..batch.num_columns())
        .map(|column| batch.column(column).as_string::<i32>())
        .collect();
    (0..batch.num_rows())
        .map(|row| {
            columns
                .iter()
                .map(|column| {
                    if column.is_null(row) {
                        "NULL"
                    } else {
                        column.value(row)
                    }
                })
                .collect::<Vec<_>>()
                .join(" | ")
        })
        .collect()
}

async fn master(session: &mut SqliteSession) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut sink = Collect::default();
    session.execute(ROWS, &mut sink).await?;
    Ok(sink.batches.iter().flat_map(cells).collect())
}

#[tokio::test]
async fn generated_ddl_recreates_what_sqlite_master_holds() -> TestResult {
    let source = tempfile::tempdir()?;
    let mut original = database(&source, FIXTURE).await?;
    let mut script = String::new();
    for relation in RELATIONS {
        let detail = original.table_detail("main", relation).await?;
        script.push_str(&SqliteDriver.table_ddl(("main", relation), &detail)?);
    }
    let copy = tempfile::tempdir()?;
    let mut rebuilt = open(&copy.path().join("copy.sqlite"), false, true)?;
    run(&mut rebuilt, &script).await?;
    let (before, after) = (master(&mut original).await?, master(&mut rebuilt).await?);
    assert_eq!(before, after);
    assert_eq!(before.len(), 9, "{before:#?}");
    assert!(before.iter().any(|row| row.contains("-- kept verbatim")));
    Ok(())
}
