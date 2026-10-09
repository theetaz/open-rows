// SPDX-License-Identifier: Apache-2.0
//! The rebuilt DDL matches the server's own: a fixture schema and a copy
//! created only from the generated definitions dump identically with
//! `pg_dump --schema-only` (M1-17).
use crate::support::{Collect, TestResult, config, connect, run};
use openrows_driver_postgres::PgSession;
use openrows_driver_postgres::ddl::create_table_sql;
use openrows_sql::{Dialect, split};
use std::process::Command;

const COPY: &str = "openrows_dump_copy";
const FUNCTION: &str =
    "CREATE FUNCTION t03.touch() RETURNS trigger LANGUAGE plpgsql AS 'BEGIN RETURN NEW; END'";

/// Tables in dependency order; every feature the DDL view rebuilds.
const TABLES: [&str; 3] = ["customers", "orders", "measurements"];

const FIXTURE: [&str; 13] = [
    "DROP SCHEMA IF EXISTS t03 CASCADE",
    "CREATE SCHEMA t03",
    FUNCTION,
    "CREATE TABLE t03.customers (id int PRIMARY KEY, email varchar(80) NOT NULL UNIQUE)",
    "CREATE TABLE t03.orders (id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY, \
     \"Note\" text DEFAULT 'n/a', price numeric(10,2) CHECK (price > 0), \
     placed timestamptz NOT NULL DEFAULT now(), tags text[], \
     customer_id int REFERENCES t03.customers(id) ON DELETE CASCADE, \
     total numeric GENERATED ALWAYS AS (price * 2) STORED)",
    "CREATE INDEX orders_note_idx ON t03.orders (lower(\"Note\")) INCLUDE (price) WHERE price > 1",
    "COMMENT ON TABLE t03.orders IS 'Customer''s orders'",
    "COMMENT ON COLUMN t03.orders.price IS 'Gross'",
    "CREATE TRIGGER orders_touch BEFORE INSERT OR UPDATE ON t03.orders \
     FOR EACH ROW EXECUTE FUNCTION t03.touch()",
    "CREATE TABLE t03.measurements (day date NOT NULL, reading int, PRIMARY KEY (day)) \
     PARTITION BY RANGE (day)",
    "CREATE TABLE t03.measurements_2024 PARTITION OF t03.measurements \
     FOR VALUES FROM ('2024-01-01') TO ('2025-01-01')",
    "CREATE TABLE t03.measurements_2025 PARTITION OF t03.measurements \
     FOR VALUES FROM ('2025-01-01') TO ('2026-01-01')",
    "CREATE INDEX measurements_reading_idx ON t03.measurements (reading)",
];

async fn script(session: &mut PgSession, sql: &str) -> TestResult {
    for statement in split(sql, Dialect::Postgres) {
        let text = statement.text(sql);
        if text.trim_start().starts_with("--") && !text.contains('\n') {
            continue;
        }
        let (result, _) = run(session, text, Collect::default()).await;
        result.map_err(|error| format!("{error}: {text}"))?;
    }
    Ok(())
}

/// `pg_dump --schema-only` of schema `t03` in `database`, without comments
/// and the per-run `\restrict` keys.
fn dump(database: &str) -> TestResult<String> {
    let container = std::env::var("OPENROWS_TEST_PG_CONTAINER")
        .map_err(|_| "OPENROWS_TEST_PG_CONTAINER is unset: run with `just test-pg`")?;
    let output = Command::new("docker")
        .args([
            "exec",
            &container,
            "pg_dump",
            "--username=postgres",
            "--schema-only",
        ])
        .args([
            "--no-owner",
            "--no-privileges",
            "--schema=t03",
            "--dbname",
            database,
        ])
        .output()?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned().into());
    }
    Ok(String::from_utf8(output.stdout)?
        .lines()
        .filter(|line| !line.starts_with("--"))
        .filter(|line| !line.starts_with("\\restrict") && !line.starts_with("\\unrestrict"))
        .collect::<Vec<_>>()
        .join("\n"))
}

#[tokio::test]
#[ignore = "needs PostgreSQL: run `just test-pg`"]
async fn generated_ddl_recreates_what_pg_dump_prints() -> TestResult {
    let mut session = connect().await?;
    for statement in FIXTURE {
        let (result, _) = run(&mut session, statement, Collect::default()).await;
        result?;
    }
    let mut definitions = Vec::new();
    for table in TABLES {
        let detail = session.table_detail("t03", table).await?;
        definitions.push(create_table_sql("t03", table, &detail)?);
    }
    for statement in [
        format!("DROP DATABASE IF EXISTS {COPY}"),
        format!("CREATE DATABASE {COPY}"),
    ] {
        let (result, _) = run(&mut session, &statement, Collect::default()).await;
        result?;
    }
    let mut copy = PgSession::connect(&openrows_driver_postgres::PgConfig {
        database: COPY.to_owned(),
        ..config()?
    })
    .await?;
    script(&mut copy, &format!("CREATE SCHEMA t03;\n{FUNCTION};")).await?;
    for definition in &definitions {
        script(&mut copy, definition).await?;
    }
    assert_eq!(dump(COPY)?, dump("postgres")?);
    assert!(definitions[2].contains("\"measurements_2025\" PARTITION OF \"t03\".\"measurements\""));
    Ok(())
}
