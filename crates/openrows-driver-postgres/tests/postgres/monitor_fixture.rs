// SPDX-License-Identifier: Apache-2.0
//! Shared helpers for the monitoring fixtures: version, cells and Arrow series.
use super::support::{self, TestResult};
use arrow_array::{Array, Float64Array, Int32Array, StringArray};
use arrow_ipc::reader::StreamReader;
use openrows_driver_postgres::PgSession;
use openrows_monitor::{LiveStore, Reading};
use std::io::Cursor;

/// `server_version_num` of the connected server, e.g. `170_006`.
pub(super) async fn server_version(session: &mut PgSession) -> TestResult<i32> {
    let batch = support::single_batch(
        session,
        "SELECT current_setting('server_version_num')::int AS version",
    )
    .await?;
    let column = support::column(&batch, "version")?;
    let version = column
        .as_any()
        .downcast_ref::<Int32Array>()
        .ok_or("version column")?;
    Ok(version.value(0))
}

/// One text cell of the first row whose `key` column equals `key`.
pub(super) fn cell<'a>(reading: &'a Reading, key: (usize, &str), column: usize) -> Option<&'a str> {
    reading
        .rows
        .iter()
        .find(|row| row.get(key.0).and_then(Option::as_deref) == Some(key.1))
        .and_then(|row| row.get(column))
        .and_then(Option::as_deref)
}

/// A numeric cell of the first row whose `key` column equals `key`; missing reads as -1.
pub(super) fn number(reading: &Reading, key: (usize, &str), column: usize) -> f64 {
    cell(reading, key, column)
        .and_then(|value| value.parse().ok())
        .unwrap_or(-1.0)
}

/// A whole-number counter cell of the first row whose `key` column equals `key`; missing
/// reads as -1.
pub(super) fn count(reading: &Reading, key: (usize, &str), column: usize) -> i64 {
    cell(reading, key, column)
        .and_then(|value| value.parse().ok())
        .unwrap_or(-1)
}

/// Every scalar series in the store's live window: name, value and validity state.
pub(super) fn series(store: &LiveStore) -> TestResult<Vec<(String, Option<f64>, String)>> {
    let bytes = store.window(None, 1_000, false)?;
    let batch = StreamReader::try_new(Cursor::new(bytes), None)?
        .next()
        .ok_or("batch")??;
    let text = |index: usize| {
        batch
            .column(index)
            .as_any()
            .downcast_ref::<StringArray>()
            .cloned()
            .ok_or("text column")
    };
    let (names, states) = (text(0)?, text(3)?);
    let values = batch
        .column(2)
        .as_any()
        .downcast_ref::<Float64Array>()
        .ok_or("values")?;
    Ok((0..batch.num_rows())
        .map(|index| {
            (
                names.value(index).to_owned(),
                (!values.is_null(index)).then(|| values.value(index)),
                states.value(index).to_owned(),
            )
        })
        .collect())
}

/// Fixture phases run in their own containers; refuse to run against the main one.
pub(super) fn require_phase(phase: &str) -> TestResult {
    if std::env::var("OPENROWS_TEST_PG_PHASE").as_deref() == Ok(phase) {
        return Ok(());
    }
    Err(format!("run this fixture through `just test-pg` (phase {phase})").into())
}
