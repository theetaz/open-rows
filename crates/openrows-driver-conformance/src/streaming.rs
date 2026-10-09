// SPDX-License-Identifier: Apache-2.0
//! [`crate::Group::Streaming`] (batching, order, early stop) and
//! [`crate::Group::Cancel`] (out-of-band cancel).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use arrow_array::RecordBatch;
use openrows_driver_api::{BatchPolicy, CancelHandle, Connection, DriverError};

use crate::dialect::{SERIES_ROWS, codes, endless, series};
use crate::sink::{Collect, ping, sqlstate};
use crate::{Failure, Target, ensure};

/// A cancelled statement must end well before this; the endless query
/// runs for hours.
const CANCEL_DEADLINE: Duration = Duration::from_secs(10);

/// Cancel requests repeat at this interval until the statement ends, so
/// the test never guesses when the server has started executing it.
const CANCEL_INTERVAL: Duration = Duration::from_millis(50);

pub(crate) async fn run(target: &Target) -> Result<(), Failure> {
    let mut connection = target.connect().await?;
    all_rows(connection.as_mut()).await?;
    early_stop(connection.as_mut()).await?;
    ping(connection.as_mut()).await?;
    connection.close().await?;
    Ok(())
}

/// Every row arrives once, in order, as one result with an early first batch.
async fn all_rows(connection: &mut dyn Connection) -> Result<(), Failure> {
    let mut sink = Collect::default();
    let summary = connection.execute(&series(), &mut sink).await?;
    ensure(sink.schemas.len() == 1, || {
        format!("{} results for one statement", sink.schemas.len())
    })?;
    let first_limit = BatchPolicy::default().first_batch_rows;
    let first = sink.batches.first().map_or(0, RecordBatch::num_rows);
    ensure(first > 0 && first <= first_limit, || {
        format!("first batch holds {first} rows (1..={first_limit})")
    })?;
    ensure(sink.batches.len() >= 2, || {
        "10 000 rows in one batch".to_owned()
    })?;
    let counted = usize::try_from(summary.rows).unwrap_or(usize::MAX);
    ensure(counted == SERIES_ROWS && sink.rows() == SERIES_ROWS, || {
        format!("summary {counted} rows, sink {} rows", sink.rows())
    })?;
    let batches = usize::try_from(summary.batches).unwrap_or(usize::MAX);
    ensure(batches == sink.batches.len(), || {
        format!("summary {batches} batches, sink {}", sink.batches.len())
    })?;
    let values = sink.column(0)?;
    let ordered = values
        .iter()
        .enumerate()
        .all(|(index, value)| value.as_deref() == Some((index + 1).to_string().as_str()));
    ensure(ordered, || "rows arrived out of order".to_owned())
}

/// A sink that stops after the first batch ends the statement cleanly.
async fn early_stop(connection: &mut dyn Connection) -> Result<(), Failure> {
    let mut sink = Collect {
        stop_after: Some(1),
        ..Collect::default()
    };
    connection.execute(&series(), &mut sink).await?;
    ensure(sink.batches.len() == 1, || {
        format!("{} batches after Stop", sink.batches.len())
    })
}

pub(crate) async fn cancel(target: &Target) -> Result<(), Failure> {
    let mut connection = target.connect().await?;
    let handle = connection
        .cancel_handle()
        .ok_or_else(|| Failure::Contract("Cancel claimed without a handle".to_owned()))?;
    let started = Instant::now();
    let done = AtomicBool::new(false);
    let sql = endless();
    let (result, cancelled) = tokio::join!(
        async {
            let result = connection.execute(&sql, &mut Collect::default()).await;
            done.store(true, Ordering::Release);
            result
        },
        cancel_until(&handle, &done),
    );
    cancelled?;
    let elapsed = started.elapsed();
    ensure(elapsed < CANCEL_DEADLINE, || {
        format!("cancelled statement ran {elapsed:?}")
    })?;
    let code = sqlstate(result, "cancelled statement")?;
    let expected = codes(target.driver.dialect()).cancelled;
    ensure(code == expected, || {
        format!("cancel SQLSTATE {code}, expected {expected}")
    })?;
    ping(connection.as_mut()).await?;
    connection.close().await?;
    Ok(())
}

/// Send cancel requests until the statement has ended.
async fn cancel_until(
    handle: &Arc<dyn CancelHandle>,
    done: &AtomicBool,
) -> Result<(), DriverError> {
    let mut ticks = tokio::time::interval(CANCEL_INTERVAL);
    let deadline = Instant::now() + CANCEL_DEADLINE;
    while !done.load(Ordering::Acquire) && Instant::now() < deadline {
        ticks.tick().await;
        handle.cancel().await?;
    }
    Ok(())
}
