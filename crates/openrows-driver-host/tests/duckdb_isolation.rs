// SPDX-License-Identifier: Apache-2.0
//! Independent processes, authoritative native file locks and explicit recovery.
#[path = "support/duckdb.rs"]
mod fixture;
#[cfg(target_os = "macos")]
#[path = "support/duckdb_image.rs"]
mod image;
use fixture::*;
use openrows_driver_api::{
    ConnectTrace, Connection, DiscardSink, Driver, DriverError, FileSource, Session,
};
use openrows_host_client::{HostSupervisor, MemoryBudget};
use openrows_host_protocol::{Control, OwnedConnect, Reply};
use std::time::Duration;
fn file(path: &std::path::Path, read_only: bool) -> FileSource {
    FileSource::Disk {
        path: path.into(),
        read_only,
        create: !read_only,
    }
}
#[tokio::test]
async fn competing_writers_and_native_other_process_lock_are_clear_errors()
-> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir_in(".")?;
    let path = std::fs::canonicalize(root.path())?.join("data.duckdb");
    let writer = open(file(&path, false)).await?;
    for read_only in [false, true] {
        assert!(
            matches!(open(file(&path, read_only)).await, Err(DriverError::Server { code, .. }) if code == "55006")
        );
    }
    Box::new(writer).close().await?;
    let mut external = HostSupervisor::new(options(MemoryBudget::default()))
        .spawn()
        .await?;
    external.send(
        1,
        &Control::Connect(OwnedConnect::try_from(&request(file(&path, false)))?),
    )?;
    loop {
        let frame = tokio::time::timeout(Duration::from_secs(5), external.recv()).await??;
        if matches!(frame.decode_control()?, Control::Reply(Reply::Connected(_))) {
            break;
        }
    }
    assert!(
        matches!(open(file(&path, false)).await, Err(DriverError::Server { code, message }) if code == "58030" && message.contains("lock"))
    );
    external.close().await?;
    let reader = open(file(&path, true)).await?;
    let other = open(file(&path, true)).await?;
    assert_ne!(reader.process_id(), other.process_id());
    Box::new(reader).close().await?;
    Box::new(other).close().await?;
    Ok(())
}
#[cfg(unix)]
#[tokio::test]
async fn killed_query_isolated_and_memory_loss_requires_fresh_connect()
-> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir_in(".")?;
    let path = std::fs::canonicalize(root.path())?.join("survivor.duckdb");
    let seed = open(file(&path, false)).await?;
    Box::new(seed).close().await?;
    let mut survivor = open(file(&path, true)).await?;
    let mut sqlite = openrows_driver_sqlite::SqliteDriver
        .connect(
            &request(memory("sqlite-survivor")),
            &mut ConnectTrace::default(),
        )
        .await?;
    let mut victim = open(memory("crash-test")).await?;
    victim
        .execute("CREATE TABLE lost_data AS SELECT 7 AS id", &mut DiscardSink)
        .await?;
    let pid = victim.process_id();
    let failure = victim.host_failure().ok_or("failure subscription")?;
    let mut sink = DiscardSink;
    let mut execution = victim.execute(
        "SELECT sum(a.i*b.i) FROM range(100000000) a(i), range(100000000) b(i)",
        &mut sink,
    );
    tokio::select! {
        result = &mut execution => { result?; return Err("query ended".into()); },
        _ = tokio::time::timeout(Duration::from_millis(50), std::future::pending::<()>()) => {}
    }
    let native_pid = rustix::process::Pid::from_raw(i32::try_from(pid)?).ok_or("pid")?;
    rustix::process::kill_process(native_pid, rustix::process::Signal::KILL)?;
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(2), execution).await?,
        Err(DriverError::Host(_))
    ));
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(2), failure).await?,
        DriverError::Host(_)
    ));
    assert!(matches!(victim.ping().await, Err(DriverError::Host(_))));
    assert_eq!(
        survivor.execute("SELECT 1", &mut DiscardSink).await?.rows,
        1
    );
    sqlite.ping().await?;
    let _ = Box::new(victim).close().await;
    let mut fresh = open(memory("crash-test")).await?;
    assert_ne!(fresh.process_id(), pid);
    assert!(
        fresh
            .server()
            .flavour
            .as_deref()
            .is_some_and(|v| v.contains("permanently loses"))
    );
    assert!(
        matches!(fresh.execute("SELECT * FROM lost_data", &mut DiscardSink).await, Err(DriverError::Server { code, .. }) if code == "42P01")
    );
    Box::new(fresh).close().await?;
    Box::new(survivor).close().await?;
    sqlite.close().await?;
    Ok(())
}
