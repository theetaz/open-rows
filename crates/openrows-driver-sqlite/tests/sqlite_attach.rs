// SPDX-License-Identifier: Apache-2.0
//! Process-shared in-memory databases (`memdb` VFS) and attached files.
mod support;

use std::path::{Path, PathBuf};

use openrows_driver_api::catalog::ObjectKind;
use openrows_driver_api::{
    Attachment, Catalog, ConnectRequest, ConnectTimeouts, ConnectTrace, Connection, Driver,
    DriverError, Endpoint, FileSource, Session, SslMode, TlsFiles,
};
use openrows_driver_sqlite::{SqliteDriver, SqliteSession, open, open_source};
use support::{Collect, TestResult, code, run};

fn memory(name: &str) -> FileSource {
    FileSource::Memory {
        name: name.to_owned(),
    }
}

fn attachment(alias: &str, path: &Path) -> Attachment {
    Attachment {
        alias: alias.to_owned(),
        path: path.to_path_buf(),
    }
}

/// A new file in `dir` holding `logs(id, line)` with one row.
async fn logs_file(dir: &tempfile::TempDir) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path = dir.path().join("logs.sqlite");
    let mut session = open(&path, false, true)?;
    run(
        &mut session,
        "CREATE TABLE logs (id INTEGER PRIMARY KEY, line TEXT); INSERT INTO logs VALUES (1, 'a')",
    )
    .await?;
    Box::new(session).close().await?;
    Ok(path)
}

async fn count(
    session: &mut SqliteSession,
    table: &str,
) -> Result<usize, Box<dyn std::error::Error>> {
    let mut sink = Collect::default();
    session
        .execute(&format!("SELECT * FROM {table}"), &mut sink)
        .await?;
    Ok(sink.rows())
}

#[tokio::test]
async fn connections_with_the_same_memory_name_share_one_database() -> TestResult {
    let mut first = open_source(&memory("openrows-shared-test"), &[])?;
    let mut second = open_source(&memory("openrows-shared-test"), &[])?;
    run(
        &mut first,
        "CREATE TABLE t (id INTEGER); INSERT INTO t VALUES (1), (2)",
    )
    .await?;
    assert_eq!(count(&mut second, "t").await?, 2);
    assert_eq!(second.server().flavour.as_deref(), Some("in-memory"));
    let mut other = open_source(&memory("openrows-other-test"), &[])?;
    assert_eq!(
        code(
            other
                .execute("SELECT * FROM t", &mut Collect::default())
                .await
        ),
        "42P01"
    );
    Box::new(first).close().await?;
    assert_eq!(
        count(&mut second, "t").await?,
        2,
        "lives while one connection is open"
    );
    Box::new(second).close().await?;
    Ok(())
}

#[tokio::test]
async fn unusable_memory_names_are_refused() {
    for name in ["", "a/b", "x?vfs=unix"] {
        assert_eq!(code(open_source(&memory(name), &[])), "58030", "{name:?}");
    }
}

#[tokio::test]
async fn attached_files_are_schemas_of_file_and_memory_databases() -> TestResult {
    let dir = tempfile::tempdir()?;
    let logs = logs_file(&dir).await?;
    let attached = [attachment("my logs", &logs)];
    let main = FileSource::Disk {
        path: dir.path().join("main.sqlite"),
        read_only: false,
        create: true,
    };
    for source in [main, memory("openrows-attach-test")] {
        let mut session = open_source(&source, &attached)?;
        assert_eq!(count(&mut session, "\"my logs\".logs").await?, 1);
        let schemas = session.list_schemas().await?;
        assert!(schemas.iter().any(|schema| schema.name == "my logs"));
        let objects = session
            .list_objects("my logs", &[ObjectKind::Table])
            .await?;
        assert_eq!(objects.first().map(|o| o.name.as_str()), Some("logs"));
        let index = session.search_index().await?;
        assert!(
            index
                .iter()
                .any(|o| o.schema == "my logs" && o.name == "logs")
        );
        Box::new(session).close().await?;
    }
    Ok(())
}

#[tokio::test]
async fn attachments_follow_the_main_read_only_mode() -> TestResult {
    let dir = tempfile::tempdir()?;
    let logs = logs_file(&dir).await?;
    let main = dir.path().join("main.sqlite");
    Box::new(open(&main, false, true)?).close().await?;
    let source = FileSource::Disk {
        path: main,
        read_only: true,
        create: false,
    };
    let mut session = open_source(&source, &[attachment("logs", &logs)])?;
    let write = session
        .execute("DELETE FROM logs.logs", &mut Collect::default())
        .await;
    assert_eq!(code(write), "25006");
    Ok(())
}

#[tokio::test]
async fn a_failing_attachment_fails_the_open_step_naming_alias_and_file() -> TestResult {
    let dir = tempfile::tempdir()?;
    let missing = dir.path().join("missing.sqlite");
    let request = ConnectRequest {
        endpoint: Endpoint::File {
            source: memory("openrows-failing-test"),
            attached: vec![attachment("archive", &missing)],
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
    assert_eq!(request.describe_endpoint(), ":memory:");
    let mut trace = ConnectTrace::default();
    let Err(DriverError::Server { code, message }) =
        SqliteDriver.connect(&request, &mut trace).await
    else {
        return Err("the connect should fail with a server error".into());
    };
    assert_eq!(code, "58030");
    assert!(
        message.contains("archive") && message.contains("missing.sqlite"),
        "{message}"
    );
    assert!(!missing.exists(), "attaching never creates a file");
    Ok(())
}
