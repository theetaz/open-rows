// SPDX-License-Identifier: Apache-2.0
//! Blocker readiness and captured chains must refer to the same executing waiter.
use super::monitor::{connection, sql};
use super::support::TestResult;
use arrow_array::StringArray;
use arrow_ipc::reader::StreamReader;
use openrows_driver_api::Connection;
use openrows_driver_postgres::PgSession;
use openrows_monitor::{LiveStore, MonitorReader, Reading, Source};
use std::{io::Cursor, time::Instant};

/// Hold a blocker until the active waiter and its redacted chain are observed.
pub(super) async fn verify(
    writer: &mut PgSession,
    monitor: &mut PgSession,
    database: &str,
) -> TestResult {
    sql(
        writer,
        "BEGIN; UPDATE monitor_fixture SET value = 'test-only' WHERE id = 1",
    )
    .await?;
    let blocker = own_pid(writer).await?;
    let mut waiter = connection(database, None).await?;
    let waiter_pid = own_pid(&mut waiter).await?;
    let pending = tokio::spawn(async move {
        sql(
            &mut waiter,
            "UPDATE monitor_fixture SET value = 'test-only' WHERE id = 1 /* test-only */",
        )
        .await
        .map_err(|_| "waiter failed")
    });
    let observed = observe(monitor, &waiter_pid, &blocker).await;
    sql(writer, "ROLLBACK").await?;
    pending.await??;
    let sample = observed?;
    assert!(sample.rows.iter().any(|row| {
        row.first().and_then(Option::as_deref) == Some(&waiter_pid)
            && row.get(1).and_then(Option::as_deref) == Some("active")
    }));
    captured_chain(sample, &blocker, &waiter_pid)
}

async fn own_pid(session: &mut PgSession) -> TestResult<String> {
    Ok(session
        .admin()
        .ok_or("admin facet")?
        .own_process()
        .await?
        .to_string())
}

async fn observe(monitor: &mut PgSession, waiter: &str, blocker: &str) -> TestResult<Reading> {
    let deadline = Instant::now();
    while deadline.elapsed().as_secs() < 5 {
        let sample = monitor.read(Source::Activity, 1_000).await;
        if sample
            .rows
            .iter()
            .any(|row| blocked_waiter(row, waiter, blocker))
        {
            return Ok(sample);
        }
    }
    Err("active blocking pair was not observed".into())
}

fn captured_chain(sample: Reading, blocker: &str, waiter: &str) -> TestResult {
    let mut store = LiveStore::default();
    store.ingest(
        Source::Activity,
        sample,
        0,
        super::monitor_capture::wall_ms(),
    );
    super::monitor_capture::capture(&store, Source::Activity)?;
    let bytes = store.window(Some(Source::Activity), 1_000, false)?;
    assert!(!String::from_utf8_lossy(&bytes).contains("test-only"));
    let batch = StreamReader::try_new(Cursor::new(bytes), None)?
        .next()
        .ok_or("batch")??;
    let chains = batch
        .column_by_name("blocking_chain")
        .ok_or("chain column")?
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or("chain type")?;
    assert!(
        chains
            .iter()
            .flatten()
            .any(|chain| chain.contains(blocker) && chain.contains(waiter))
    );
    Ok(())
}

fn blocked_waiter(row: &[Option<String>], waiter: &str, blocker: &str) -> bool {
    // Activity state is transaction-cached; wait events and blocking PIDs are live reads.
    // A fresh guarded sample must show the waiter executing before releasing the blocker.
    row.first().and_then(Option::as_deref) == Some(waiter)
        && row.get(1).and_then(Option::as_deref) == Some("active")
        && row
            .get(2)
            .and_then(Option::as_deref)
            .is_some_and(|wait| wait.starts_with("Lock: "))
        && row.get(5).and_then(Option::as_deref).is_some_and(|ids| {
            ids.trim_matches(['{', '}'])
                .split(',')
                .any(|pid| pid == blocker)
        })
}

#[test]
fn blocking_readiness_rejects_idle_state_with_live_blockers() {
    let mut row: Vec<_> = ["2", "idle", "Lock: transactionid", "0", "0", "{1}"]
        .into_iter()
        .map(|value| Some(value.to_owned()))
        .collect();
    assert!(!blocked_waiter(&row, "2", "1"));
    row[1] = Some("active".into());
    assert!(blocked_waiter(&row, "2", "1"));
    row[5] = Some("{11}".into());
    assert!(!blocked_waiter(&row, "2", "1"));
    row[5] = Some("{1}".into());
    row[2] = Some("Client: ClientRead".into());
    assert!(!blocked_waiter(&row, "2", "1"));
}
