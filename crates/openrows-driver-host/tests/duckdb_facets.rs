// SPDX-License-Identifier: Apache-2.0
//! Catalog, edits and user transactions share the one hosted `DuckDB` session.
#[path = "support/duckdb.rs"]
mod fixture;
#[cfg(target_os = "macos")]
#[path = "support/duckdb_image.rs"]
mod image;
use fixture::*;
use openrows_driver_api::catalog::ObjectKind;
use openrows_driver_api::{DiscardSink, DriverError, Session, catalog, editor};
use openrows_sql::select::BoundSql;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[tokio::test]
async fn user_transaction_is_tracked_and_guards_edits() -> TestResult {
    let mut connection = open(memory("facets-transaction")).await?;
    connection
        .execute(
            "CREATE TABLE t (id INTEGER PRIMARY KEY, note TEXT)",
            &mut DiscardSink,
        )
        .await?;
    assert!(!connection.in_transaction());
    connection.execute("BEGIN", &mut DiscardSink).await?;
    connection
        .execute("CREATE TABLE pending (id INTEGER)", &mut DiscardSink)
        .await?;
    assert!(connection.in_transaction());
    let seen = catalog(&mut connection)?
        .list_objects("main", &[ObjectKind::Table])
        .await?;
    assert!(seen.iter().any(|object| object.name == "pending"));
    let edit = [BoundSql {
        sql: "INSERT INTO t (id) VALUES ($1)".into(),
        params: vec![Some("1".into())],
    }];
    let refused = editor(&mut connection)?.commit_edits(&edit).await;
    assert!(
        matches!(refused, Err(DriverError::Unsupported(_))),
        "{refused:?}"
    );
    connection.execute("ROLLBACK", &mut DiscardSink).await?;
    assert!(!connection.in_transaction());
    editor(&mut connection)?.commit_edits(&edit).await?;
    let info = editor(&mut connection)?.row_edit_info("main", "t").await?;
    assert_eq!(info.key, ["id"]);
    let names = catalog(&mut connection)?
        .search_index()
        .await?
        .into_iter()
        .map(|object| object.name)
        .collect::<Vec<_>>();
    assert_eq!(names, ["t"]);
    Box::new(connection).close().await?;
    Ok(())
}

#[tokio::test]
async fn keyless_tables_have_no_edit_key() -> TestResult {
    let mut connection = open(memory("facets-keyless")).await?;
    connection
        .execute(
            "CREATE TABLE loose (a INTEGER, b INTEGER UNIQUE); \
             CREATE TABLE keyed (a INTEGER NOT NULL UNIQUE, b TEXT)",
            &mut DiscardSink,
        )
        .await?;
    let facet = editor(&mut connection)?;
    assert!(facet.row_edit_info("main", "loose").await?.key.is_empty());
    assert_eq!(facet.row_edit_info("main", "keyed").await?.key, ["a"]);
    let databases = catalog(&mut connection)?.list_databases().await?;
    assert_eq!(
        databases.first().map(|item| item.name.as_str()),
        Some("main")
    );
    Box::new(connection).close().await?;
    Ok(())
}

#[tokio::test]
async fn read_only_files_list_but_refuse_edits() -> TestResult {
    let root = tempfile::tempdir_in(".")?;
    let path = std::fs::canonicalize(root.path())?.join("facets.duckdb");
    let mut writer = open(openrows_driver_api::FileSource::Disk {
        path: path.clone(),
        read_only: false,
        create: true,
    })
    .await?;
    writer
        .execute("CREATE TABLE t (id INTEGER PRIMARY KEY)", &mut DiscardSink)
        .await?;
    Box::new(writer).close().await?;
    let mut read = open(openrows_driver_api::FileSource::Disk {
        path,
        read_only: true,
        create: false,
    })
    .await?;
    let databases = catalog(&mut read)?.list_databases().await?;
    assert_eq!(databases[0].comment.as_deref(), Some("read-only"));
    let edit = [BoundSql {
        sql: "INSERT INTO t (id) VALUES ($1)".into(),
        params: vec![Some("1".into())],
    }];
    let refused = editor(&mut read)?.commit_edits(&edit).await;
    assert!(
        matches!(refused, Err(DriverError::Server { .. })),
        "{refused:?}"
    );
    assert!(!read.in_transaction());
    Box::new(read).close().await?;
    Ok(())
}
