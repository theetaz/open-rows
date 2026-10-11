// SPDX-License-Identifier: Apache-2.0
//! A fixture administrator resets its isolated database, never a collector.
use super::support::{Collect, TestResult};
use arrow_array::{Array, Float64Array, StringArray};
use arrow_ipc::reader::StreamReader;
use openrows_driver_api::Session;
use openrows_driver_postgres::PgSession;
use openrows_monitor::{LiveStore, MonitorReader, Source};
use std::{io::Cursor, time::Instant};

/// A real database reset ends rates with null counter-reset observations.
pub(super) async fn verify(monitor: &mut PgSession, database: &str) -> TestResult {
    let before = monitor.read(Source::Database, 1_000).await;
    let mut administrator = super::monitor::connection(database, None).await?;
    administrator
        .execute("SELECT pg_stat_reset()", &mut Collect::default())
        .await?;
    let deadline = Instant::now();
    let mut after = monitor.read(Source::Database, 1_000).await;
    while after.epoch == before.epoch && deadline.elapsed().as_secs() < 5 {
        after = monitor.read(Source::Database, 1_000).await;
    }
    assert_ne!(before.epoch, after.epoch);
    let mut store = LiveStore::default();
    store.ingest(Source::Database, before, 0, 0.0);
    store.ingest(Source::Database, after, 1_000, 1_000.0);
    let bytes = store.window(None, 1_000, false)?;
    let batch = StreamReader::try_new(Cursor::new(bytes), None)?
        .next()
        .ok_or("batch")??;
    let states = batch
        .column(3)
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or("states")?;
    let values = batch
        .column(2)
        .as_any()
        .downcast_ref::<Float64Array>()
        .ok_or("values")?;
    let mut resets = 0;
    for (index, state) in states.iter().enumerate() {
        if state == Some("counterReset") {
            resets += 1;
            assert!(values.is_null(index));
        }
    }
    assert!(resets > 0);
    Ok(())
}
