// SPDX-License-Identifier: Apache-2.0
//! M1-25 export through the session actor against a real `MySQL`/`MariaDB`
//! server: a whole-table SQL `INSERT` export re-imports to an identical table.
//!
//! Ignored by `just test`; `just test-mysql` starts throwaway containers and
//! runs them with `--run-ignored only`.
use openrows_core::export::{ExportJob, ExportOutcome, export_statement};
use openrows_core::script::ScriptCanceller;
use openrows_core::{QueryEvent, SessionManager};
use openrows_driver_api::{
    ConnectRequest, ConnectTimeouts, ConnectTrace, Endpoint, Session, SslMode, TlsFiles,
};
use openrows_export::{CsvOptions, Delimiter, Exporter, Format, InsertOptions};
use openrows_net::SecretString;
use openrows_results::ResultStore;
use openrows_sql::Dialect;
use openrows_sql::select::{BoundSql, TableRef, all_rows_sql};
use openrows_types::SessionId;
use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::Arc;

#[path = "mysql/explain.rs"]
mod explain;
#[path = "mysql/limits.rs"]
mod limits;
#[path = "mysql/resilience.rs"]
mod resilience;
#[path = "mysql/table_filters.rs"]
mod table_filters;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const DATABASE: &str = "openrows_test";

const COLUMNS: &str = "(id int PRIMARY KEY, txt text, bin varbinary(16), num decimal(30,5), \
    f double, d date, dt datetime(6), j json, b tinyint(1), u bigint unsigned)";

const ROWS: &str = "(1, 'it''s \\\\ \"q\" ünï 😀\\nnext\\0end', X'00FF27', -12345678901234567890.12345, \
    0.1, '2024-02-29', '2024-02-29 12:34:56.123456', '{\"a\": [1, \"x\"]}', 1, 18446744073709551615), \
    (2, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL), \
    (3, '', X'', 0, -2.5e-300, '1000-01-01', '1970-01-01 00:00:00', 'null', 0, 0)";

fn request() -> TestResult<ConnectRequest> {
    let variable = |name: &str| {
        std::env::var(name).map_err(|_| format!("{name} is unset: run with `just test-mysql`"))
    };
    Ok(ConnectRequest {
        endpoint: Endpoint::Server {
            host: "127.0.0.1".to_owned(),
            port: variable("OPENROWS_TEST_MYSQL_PORT")?.parse()?,
        },
        database: DATABASE.to_owned(),
        user: "root".to_owned(),
        password: Some(SecretString::from(
            variable("OPENROWS_TEST_MYSQL_PASSWORD")?.as_str(),
        )),
        application_name: "openrows-test".to_owned(),
        ssl_mode: SslMode::Prefer,
        tls: TlsFiles::default(),
        via: None,
        timeouts: ConnectTimeouts::default(),
        startup_commands: Vec::new(),
    })
}

async fn open(manager: &SessionManager) -> TestResult<SessionId> {
    let request = request()?;
    Ok(manager
        .open(async move {
            let session =
                openrows_driver_mysql::connect(&request, &mut ConnectTrace::default()).await?;
            let session: Box<dyn Session> = Box::new(session);
            Ok(session)
        })
        .await?)
}

async fn run(manager: &SessionManager, session: SessionId, sql: &str) -> TestResult {
    let mut handle = manager.execute(session, sql)?;
    while let Some(event) = handle.next_event().await {
        if let QueryEvent::Failed { error } = event {
            return Err(format!("{sql}: {}", error.message).into());
        }
    }
    Ok(())
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

/// Row count and a CRC sum over every quoted value, read through a CSV export.
async fn checksum(manager: &SessionManager, session: SessionId, table: &str) -> TestResult<String> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("sum.csv");
    let sql = format!(
        "SELECT COUNT(*), SUM(CRC32(CONCAT_WS('|', QUOTE(id), QUOTE(txt), QUOTE(HEX(bin)), \
         QUOTE(num), QUOTE(f), QUOTE(d), QUOTE(dt), QUOTE(j), QUOTE(b), QUOTE(u)))) FROM {table}"
    );
    let statement = BoundSql {
        sql,
        params: Vec::new(),
    };
    let csv = Format::Csv(CsvOptions {
        delimiter: Delimiter::Comma,
        header: false,
        ..CsvOptions::default()
    });
    export(manager, session, (statement, &csv), &path).await?;
    Ok(std::fs::read_to_string(path)?)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs MySQL or MariaDB; run with `just test-mysql`"]
async fn table_export_round_trips_through_sql_insert() -> TestResult {
    let manager = SessionManager::new(Arc::new(ResultStore::default()));
    let session = open(&manager).await?;
    for sql in [
        "DROP TABLE IF EXISTS m125_src, m125_copy".to_owned(),
        format!("CREATE TABLE m125_src {COLUMNS}"),
        format!("CREATE TABLE m125_copy {COLUMNS}"),
        format!("INSERT INTO m125_src VALUES {ROWS}"),
        "INSERT INTO m125_src (id, txt, num, f, b) WITH RECURSIVE n(i) AS \
         (SELECT 4 UNION ALL SELECT i + 1 FROM n WHERE i < 1000) \
         SELECT i, CONCAT('row ', i), i / 7, i / 3, i % 2 FROM n"
            .to_owned(),
    ] {
        run(&manager, session, &sql).await?;
    }
    let directory = tempfile::tempdir()?;
    let dump = directory.path().join("src.sql");
    let table = TableRef {
        schema: DATABASE.to_owned(),
        name: "m125_src".to_owned(),
    };
    let statement = all_rows_sql(Dialect::MySql, &table, &["id".to_owned()])?;
    let insert = Format::Insert(InsertOptions {
        dialect: Dialect::MySql,
        table: "m125_copy".to_owned(),
        rows_per_statement: NonZeroUsize::new(250).ok_or("zero")?,
    });
    assert_eq!(
        export(&manager, session, (statement, &insert), &dump).await?,
        1000
    );
    let script = std::fs::read_to_string(&dump)?;
    for statement in openrows_sql::split(&script, Dialect::MySql) {
        run(&manager, session, statement.text(&script)).await?;
    }
    let source = checksum(&manager, session, "m125_src").await?;
    assert!(source.starts_with("1000,"), "{source}");
    assert_eq!(source, checksum(&manager, session, "m125_copy").await?);
    run(&manager, session, "DROP TABLE m125_src, m125_copy").await?;
    Ok(())
}
