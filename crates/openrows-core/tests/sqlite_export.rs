// SPDX-License-Identifier: Apache-2.0
//! Export through the session actor and the real `SQLite` driver: the SQL
//! `INSERT` file re-imports to an identical table, CSV and JSON match.
use openrows_core::SessionManager;
use openrows_core::export::{ExportJob, ExportOutcome, export_statement};
use openrows_core::script::ScriptCanceller;
use openrows_driver_api::Session;
use openrows_export::{CsvOptions, Delimiter, Exporter, Format, InsertOptions, JsonLayout};
use openrows_results::ResultStore;
use openrows_sql::Dialect;
use openrows_sql::select::BoundSql;
use openrows_types::SessionId;
use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::Arc;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const SETUP: &[&str] = &[
    "CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT, score REAL, data BLOB)",
    "INSERT INTO t VALUES (1, 'it''s \"q\"', 1.5, X'00ff'), (2, NULL, NULL, NULL), \
     (3, 'ünï, 😀' || char(10) || 'x', -0.25, X'')",
    "WITH RECURSIVE n(i) AS (SELECT 4 UNION ALL SELECT i + 1 FROM n WHERE i < 1000) \
     INSERT INTO t SELECT i, 'row ' || i, i / 4.0, NULL FROM n",
];

async fn open(manager: &SessionManager, path: &Path) -> TestResult<SessionId> {
    let path = path.to_path_buf();
    Ok(manager
        .open(async move {
            let session: Box<dyn Session> =
                Box::new(openrows_driver_sqlite::open(&path, false, true)?);
            Ok(session)
        })
        .await?)
}

async fn run(manager: &SessionManager, session: SessionId, sql: &str) -> TestResult {
    let mut handle = manager.execute(session, sql)?;
    while let Some(event) = handle.next_event().await {
        if let openrows_core::QueryEvent::Failed { error } = event {
            return Err(error.message.into());
        }
    }
    Ok(())
}

async fn export(
    manager: &SessionManager,
    session: SessionId,
    (sql, format): (&str, &Format),
    path: &Path,
) -> TestResult<u64> {
    let job = ExportJob {
        exporter: Exporter::create(path, format)?,
        cancel: ScriptCanceller::default(),
        progress: Box::new(|_| {}),
    };
    let statement = BoundSql {
        sql: sql.to_owned(),
        params: Vec::new(),
    };
    match export_statement(manager, session, statement, job).await {
        ExportOutcome::Written { rows } => Ok(rows),
        other => Err(format!("export failed: {other:?}").into()),
    }
}

/// Every column of `table` as one text digest, ordered by id.
async fn digest(manager: &SessionManager, session: SessionId, table: &str) -> TestResult<String> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("digest.csv");
    let sql = format!("SELECT id, name, score, quote(data) FROM {table} ORDER BY id");
    let csv = Format::Csv(CsvOptions {
        delimiter: Delimiter::Comma,
        header: false,
    });
    export(manager, session, (&sql, &csv), &path).await?;
    Ok(std::fs::read_to_string(path)?)
}

#[tokio::test]
async fn sql_insert_export_round_trips_and_text_formats_match() -> TestResult {
    let manager = SessionManager::new(Arc::new(ResultStore::default()));
    let directory = tempfile::tempdir()?;
    let session = open(&manager, &directory.path().join("db.sqlite")).await?;
    for sql in SETUP {
        run(&manager, session, sql).await?;
    }
    let dump = directory.path().join("t.sql");
    let insert = Format::Insert(InsertOptions {
        dialect: Dialect::Sqlite,
        table: "copy".to_owned(),
        rows_per_statement: NonZeroUsize::new(100).ok_or("zero")?,
    });
    let rows = export(&manager, session, ("SELECT * FROM t", &insert), &dump).await?;
    assert_eq!(rows, 1000);
    run(
        &manager,
        session,
        "CREATE TABLE copy (id INTEGER PRIMARY KEY, name TEXT, score REAL, data BLOB)",
    )
    .await?;
    let script = std::fs::read_to_string(&dump)?;
    for statement in openrows_sql::split(&script, Dialect::Sqlite) {
        run(&manager, session, statement.text(&script)).await?;
    }
    assert_eq!(
        digest(&manager, session, "t").await?,
        digest(&manager, session, "copy").await?
    );

    let csv = directory.path().join("t.csv");
    let options = Format::Csv(CsvOptions {
        delimiter: Delimiter::Comma,
        header: true,
    });
    export(
        &manager,
        session,
        ("SELECT * FROM t WHERE id <= 3 ORDER BY id", &options),
        &csv,
    )
    .await?;
    assert_eq!(
        std::fs::read_to_string(&csv)?,
        "id,name,score,data\r\n1,\"it's \"\"q\"\"\",1.5,\\x00ff\r\n2,,,\r\n3,\"ünï, 😀\nx\",-0.25,\\x\r\n"
    );
    let json = directory.path().join("t.json");
    let lines = Format::Json(JsonLayout::Lines);
    export(
        &manager,
        session,
        ("SELECT id, name FROM t WHERE id <= 2 ORDER BY id", &lines),
        &json,
    )
    .await?;
    assert_eq!(
        std::fs::read_to_string(&json)?,
        "{\"id\":1,\"name\":\"it's \\\"q\\\"\"}\n{\"id\":2,\"name\":null}\n"
    );
    Ok(())
}
