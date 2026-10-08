// SPDX-License-Identifier: Apache-2.0
//! Opening files, streaming, binding, cancel and read-only on real files.
mod support;

use std::time::Duration;

use arrow_array::{Array, Int64Array, StringArray};
use arrow_schema::DataType;
use openrows_driver_api::{
    ConnectRequest, ConnectStage, ConnectTimeouts, ConnectTrace, Driver, Endpoint, Session,
    SslMode, TlsFiles,
};
use openrows_driver_sqlite::{SqliteDriver, open};
use support::{Collect, TestResult, code, database, run};

const SETUP: &str = "CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT, score REAL);
    WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 1000)
    INSERT INTO t SELECT i, 'row ' || i, i / 2.0 FROM n";

#[tokio::test]
async fn rows_stream_in_typed_batches_with_an_early_first_batch() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut session = database(&dir, SETUP).await?;
    let mut sink = Collect::default();
    let summary = session
        .execute("SELECT id, name, score FROM t ORDER BY id", &mut sink)
        .await?;
    assert_eq!((summary.rows, sink.rows()), (1000, 1000));
    assert_eq!(summary.command_tag, "SELECT 1000");
    assert_eq!(
        sink.batches[0].num_rows(),
        200,
        "first batch flushes after 200 rows"
    );
    assert!(sink.batches.len() >= 2);
    let types: Vec<DataType> = sink.schemas[0]
        .fields()
        .iter()
        .map(|f| f.data_type().clone())
        .collect();
    assert_eq!(
        types,
        vec![DataType::Int64, DataType::Utf8, DataType::Float64]
    );
    let names = sink.batches[0]
        .column(1)
        .as_any()
        .downcast_ref::<StringArray>()
        .map(|a| a.value(0).to_owned());
    assert_eq!(names.as_deref(), Some("row 1"));
    let mut stopped = Collect {
        stop_after: Some(1),
        ..Collect::default()
    };
    session.execute("SELECT * FROM t", &mut stopped).await?;
    assert_eq!(stopped.batches.len(), 1, "a Stop ends the stream");
    Ok(())
}

#[tokio::test]
async fn parameters_bind_as_text_and_dml_reports_changes() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut session = database(&dir, SETUP).await?;
    let mut sink = Collect::default();
    let params = [Some("5".to_owned()), None];
    session
        .execute_bound(
            "SELECT id, ?2 IS NULL FROM t WHERE id = ?1",
            &params,
            &mut sink,
        )
        .await?;
    let id = sink.batches[0]
        .column(0)
        .as_any()
        .downcast_ref::<Int64Array>()
        .map(|a| a.value(0));
    assert_eq!(id, Some(5), "text '5' compares as the integer key");
    let update = session
        .execute_bound(
            "UPDATE t SET name = ?1 WHERE id <= 3",
            &[Some("x".to_owned())],
            &mut Collect::default(),
        )
        .await?;
    assert_eq!(update.command_tag, "UPDATE 3");
    let wrong = session
        .execute_bound("SELECT ?1", &[], &mut Collect::default())
        .await;
    assert_eq!(code(wrong), "07001");
    let empty = &mut Collect::default();
    session
        .execute("SELECT * FROM t WHERE id < 0", empty)
        .await?;
    assert_eq!(
        (empty.schemas.len(), empty.rows()),
        (1, 0),
        "an empty result still has a schema"
    );
    Ok(())
}

#[tokio::test]
async fn cancel_interrupts_a_long_query_and_the_session_stays_usable() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut session = database(&dir, "SELECT 1").await?;
    let cancel = session.cancel_handle().ok_or("no cancel handle")?;
    let canceller = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        cancel.cancel().await
    });
    let endless =
        "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n) SELECT count(*) FROM n";
    let started = std::time::Instant::now();
    let result = session.execute(endless, &mut Collect::default()).await;
    canceller.await??;
    assert_eq!(code(result), "57014");
    assert!(started.elapsed() < Duration::from_secs(5));
    let after = session.execute("SELECT 1", &mut Collect::default()).await?;
    assert_eq!(after.rows, 1);
    Ok(())
}

#[tokio::test]
async fn read_only_files_refuse_writes_and_missing_files_are_not_created() -> TestResult {
    let dir = tempfile::tempdir()?;
    drop(database(&dir, SETUP).await?);
    let path = dir.path().join("test.sqlite");
    let mut read_only = open(&path, true, false)?;
    let write = read_only
        .execute("DELETE FROM t", &mut Collect::default())
        .await;
    assert_eq!(code(write), "25006");
    let read = run(&mut read_only, "SELECT count(*) FROM t").await?;
    assert_eq!(read.rows, 1);
    let missing = dir.path().join("missing.sqlite");
    assert_eq!(code(open(&missing, false, false)), "58030");
    assert!(!missing.exists());
    let not_a_database = dir.path().join("notes.txt");
    std::fs::write(
        &not_a_database,
        "plain text, long enough to look like a header....",
    )?;
    assert_eq!(code(open(&not_a_database, false, false)), "58030");
    Ok(())
}

#[tokio::test]
async fn the_driver_opens_files_and_records_its_stages() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("new.sqlite");
    let request = ConnectRequest {
        endpoint: Endpoint::File {
            path: path.clone(),
            read_only: false,
            create: true,
        },
        database: "main".to_owned(),
        user: String::new(),
        password: None,
        application_name: "OpenRows".to_owned(),
        ssl_mode: SslMode::Disable,
        tls: TlsFiles::default(),
        via: None,
        timeouts: ConnectTimeouts::default(),
    };
    let mut trace = ConnectTrace::default();
    let connection = SqliteDriver.connect(&request, &mut trace).await?;
    assert!(path.exists(), "create opens a new file");
    let server = connection.server();
    assert_eq!(
        (server.product.as_str(), server.database.as_str()),
        ("SQLite", "main")
    );
    assert!(server.version.starts_with('3'));
    assert_eq!(
        trace.completed.first().map(|(stage, _)| *stage),
        Some(ConnectStage::Connect)
    );
    connection.close().await?;
    assert_eq!(SqliteDriver.dialect(), openrows_sql::Dialect::Sqlite);
    assert_eq!(SqliteDriver.home_database(), "main");
    Ok(())
}
