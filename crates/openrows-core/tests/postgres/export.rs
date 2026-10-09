// SPDX-License-Identifier: Apache-2.0
//! M1-25 export against a real server: a whole-table SQL `INSERT` export
//! re-imports to an identical table (server-side checksum) and a large CSV
//! export streams every row without touching the result store.
use super::{CountRows, TestResult, events, open};
use openrows_core::export::{ExportJob, ExportOutcome, export_statement};
use openrows_core::script::ScriptCanceller;
use openrows_core::{QueryEvent, SessionManager};
use openrows_export::{CsvOptions, Delimiter, Exporter, Format, InsertOptions};
use openrows_sql::Dialect;
use openrows_sql::select::{BoundSql, TableRef, all_rows_sql};
use openrows_types::SessionId;
use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::Ordering;

const COLUMNS: &str = "(id int PRIMARY KEY, txt text, bin bytea, flag bool, num numeric(12,3), \
    big numeric, f8 float8, f4 real, d date, ts timestamp, tz timestamptz, u uuid, j jsonb, arr int[])";

const ROWS: [&str; 3] = [
    "(1, E'it''s \\\\ \"q\" ünï 😀\\nnext', '\\x00ff27'::bytea, true, -12.345, \
     123456789012345678901234567890.5, 'NaN', 0.1, '2024-02-29', '2024-02-29 12:34:56.123456', \
     '2024-02-29 12:34:56+02', 'a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11', '{\"a\": [1, \"x\"]}', '{1,2}')",
    "(2, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL)",
    "(3, '', '', false, 0, 'NaN', '-Infinity', 'Infinity', '0001-01-01', 'infinity', \
     '1970-01-01 00:00:00+00', '00000000-0000-0000-0000-000000000000', 'null', '{}')",
];

async fn run(manager: &SessionManager, session: SessionId, sql: &str) -> TestResult {
    let mut handle = manager.execute(session, sql)?;
    match events(&mut handle).await?.pop() {
        Some(QueryEvent::Completed { .. }) => Ok(()),
        other => Err(format!("{sql}: {other:?}").into()),
    }
}

async fn export(
    manager: &SessionManager,
    session: SessionId,
    (statement, format): (BoundSql, &Format),
    path: &Path,
) -> TestResult<u64> {
    let job = ExportJob {
        exporter: Exporter::create(path, format)?,
        cancel: ScriptCanceller::default(),
        progress: Box::new(|_| {}),
    };
    match export_statement(manager, session, statement, job).await {
        ExportOutcome::Written { rows } => Ok(rows),
        other => Err(format!("export failed: {other:?}").into()),
    }
}

fn plain(sql: &str) -> BoundSql {
    BoundSql {
        sql: sql.to_owned(),
        params: Vec::new(),
    }
}

/// The server's md5 over every row's text form, read through a CSV export.
async fn checksum(manager: &SessionManager, session: SessionId, table: &str) -> TestResult<String> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("sum.csv");
    let sql = format!("SELECT md5(string_agg(t::text, '|' ORDER BY t.id)) FROM m125.{table} t");
    let csv = Format::Csv(CsvOptions {
        delimiter: Delimiter::Comma,
        header: false,
    });
    export(manager, session, (plain(&sql), &csv), &path).await?;
    Ok(std::fs::read_to_string(path)?)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs PostgreSQL; run with `just test-pg`"]
async fn table_export_round_trips_through_sql_insert() -> TestResult {
    let store = Arc::new(CountRows::default());
    let manager = SessionManager::new(store.clone());
    let session = open(&manager).await?;
    for sql in [
        "DROP SCHEMA IF EXISTS m125 CASCADE".to_owned(),
        "CREATE SCHEMA m125".to_owned(),
        format!("CREATE TABLE m125.src {COLUMNS}"),
        format!("CREATE TABLE m125.copy {COLUMNS}"),
        format!("INSERT INTO m125.src VALUES {}", ROWS.join(", ")),
        "INSERT INTO m125.src (id, txt, flag, num, f8) SELECT g, 'row ' || g, g % 2 = 0, g / 7.0, \
         g / 3.0 FROM generate_series(4, 5000) g"
            .to_owned(),
        // Late in the scan: the driver continues in a new result with `d` as text.
        "INSERT INTO m125.src (id, d) VALUES (6000, 'infinity')".to_owned(),
    ] {
        run(&manager, session, &sql).await?;
    }
    let directory = tempfile::tempdir()?;
    let dump = directory.path().join("src.sql");
    let table = TableRef {
        schema: "m125".to_owned(),
        name: "src".to_owned(),
    };
    let statement = all_rows_sql(Dialect::Postgres, &table, &["id".to_owned()])?;
    let insert = Format::Insert(InsertOptions {
        dialect: Dialect::Postgres,
        table: "copy".to_owned(),
        rows_per_statement: NonZeroUsize::new(500).ok_or("zero")?,
    });
    assert_eq!(
        export(&manager, session, (statement, &insert), &dump).await?,
        5001
    );
    let script = std::fs::read_to_string(&dump)?;
    run(&manager, session, "SET search_path TO m125").await?;
    for statement in openrows_sql::split(&script, Dialect::Postgres) {
        run(&manager, session, statement.text(&script)).await?;
    }
    run(&manager, session, "RESET search_path").await?;
    assert_eq!(
        checksum(&manager, session, "src").await?,
        checksum(&manager, session, "copy").await?
    );
    assert_eq!(
        store.0.load(Ordering::Relaxed),
        0,
        "exports bypass the result store"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs PostgreSQL; run with `just test-pg`"]
async fn a_large_csv_export_matches_the_server_checksum() -> TestResult {
    let manager = SessionManager::new(Arc::new(CountRows::default()));
    let session = open(&manager).await?;
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("series.csv");
    let csv = Format::Csv(CsvOptions {
        delimiter: Delimiter::Semicolon,
        header: true,
    });
    let sql = "SELECT g AS id, 'v;' || g AS label FROM generate_series(1, 300000) g";
    assert_eq!(
        export(&manager, session, (plain(sql), &csv), &path).await?,
        300_000
    );
    let text = std::fs::read_to_string(&path)?;
    let mut lines = text.split("\r\n");
    assert_eq!(lines.next(), Some("id;label"));
    let mut sum = 0u64;
    for line in lines.filter(|line| !line.is_empty()) {
        let (id, label) = line.split_once(';').ok_or("missing delimiter")?;
        assert_eq!(label, format!("\"v;{id}\""));
        sum += id.parse::<u64>()?;
    }
    assert_eq!(sum, 300_000 * 300_001 / 2);
    Ok(())
}
