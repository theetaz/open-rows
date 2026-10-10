// SPDX-License-Identifier: Apache-2.0
//! Non-default, deliberately destructive disposable-child test hooks.
use crate::{flow::Flow, output::Output, sink::Sink};
use arrow_array::{ArrayRef, BinaryArray, Decimal128Array, Int64Array, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};
use openrows_driver_api::{DriverError, ResultSink, SinkFlow};
use openrows_host_protocol::{Reply, WireSummary};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
static HEARTBEAT: AtomicBool = AtomicBool::new(true);
pub(crate) fn heartbeating() -> bool {
    HEARTBEAT.load(Ordering::SeqCst)
}
#[allow(
    clippy::panic,
    reason = "non-default test hook deliberately tests worker panic isolation"
)]
pub(crate) fn execute(
    sql: &str,
    output: &Output,
    flow: &Arc<Flow>,
    authorized: &Arc<AtomicU64>,
) -> Option<Result<Reply, DriverError>> {
    match sql {
        "OPENROWS_TEST_FATAL_ALLOC_FAILURE" | "OPENROWS_TEST_ABORT" => std::process::abort(),
        "OPENROWS_TEST_SEGFAULT" => {
            #[cfg(unix)]
            {
                let _ = rustix::process::kill_process(
                    rustix::process::getpid(),
                    rustix::process::Signal::SEGV,
                );
            }
            std::process::abort();
        }
        "OPENROWS_TEST_PANIC" => panic!("test-only worker panic"),
        "OPENROWS_TEST_ALLOC_FAILURE" => Some(Err(DriverError::Server {
            code: "53200".into(),
            message: "test-only allocation refused".into(),
        })),
        "OPENROWS_TEST_MEMORY" => {
            let mut allocation = vec![0_u8; 272 * 1024 * 1024];
            for page in allocation.chunks_mut(4096) {
                page[0] = 1;
            }
            std::hint::black_box(&allocation);
            std::thread::park();
            Some(Err(DriverError::Host(
                openrows_driver_api::HostFailure::MemoryLimit,
            )))
        }
        "OPENROWS_TEST_NO_HEARTBEAT" => {
            HEARTBEAT.store(false, Ordering::SeqCst);
            Some(Ok(Reply::Pong))
        }
        "OPENROWS_TEST_HANG" => {
            std::thread::park();
            Some(Ok(Reply::Pong))
        }
        _ if sql.starts_with("OPENROWS_TEST_LARGE ") => Some(
            sql[20..]
                .parse::<usize>()
                .map_err(|_| invalid())
                .and_then(fixture_large)
                .and_then(|batch| deliver(batch, output, flow, authorized)),
        ),
        _ => sql
            .strip_prefix("OPENROWS_TEST_BATCH ")
            .map(|shape| synthetic(shape, output, flow, authorized)),
    }
}
fn synthetic(
    shape: &str,
    output: &Output,
    flow: &Arc<Flow>,
    authorized: &Arc<AtomicU64>,
) -> Result<Reply, DriverError> {
    let (rows, columns) = shape.split_once(' ').ok_or_else(invalid)?;
    let rows = rows.parse::<usize>().map_err(|_| invalid())?;
    let columns = columns.parse::<usize>().map_err(|_| invalid())?;
    deliver(fixture(rows, columns)?, output, flow, authorized)
}
fn deliver(
    batch: RecordBatch,
    output: &Output,
    flow: &Arc<Flow>,
    authorized: &Arc<AtomicU64>,
) -> Result<Reply, DriverError> {
    let rows = batch.num_rows();
    let mut sink = Sink::new(output.clone(), flow.clone(), authorized.clone());
    sink.begin_result(batch.schema());
    let delivered = sink.push_batch(batch) == SinkFlow::Continue;
    sink.finish().map_err(|_| invalid())?;
    if sink.error.is_some() {
        return Err(invalid());
    }
    Ok(Reply::Executed(WireSummary {
        rows: if delivered { rows as u64 } else { 0 },
        batches: u64::from(delivered),
        result: sink.result,
        sequence: sink.sequence,
        ..WireSummary::default()
    }))
}
/// Identical synthetic producer used by paired subprocess benchmarks.
/// # Errors
/// Shape exceeds the capped disposable test workload or Arrow rejects a type.
pub fn fixture(rows: usize, columns: usize) -> Result<RecordBatch, DriverError> {
    if rows > 65_536 || columns > 200 || columns == 0 {
        return Err(invalid());
    }
    let fields = (0..columns)
        .map(|column| {
            Field::new(
                format!("c{column}"),
                match column % 4 {
                    0 => DataType::Int64,
                    1 => DataType::Decimal128(20, 4),
                    2 => DataType::Utf8,
                    _ => DataType::Binary,
                },
                true,
            )
        })
        .collect::<Vec<_>>();
    let arrays = (0..columns)
        .map(|column| column_array(rows, column))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RecordBatch::try_new(Arc::new(Schema::new(fields)), arrays)?)
}
fn column_array(rows: usize, column: usize) -> Result<ArrayRef, DriverError> {
    Ok(match column % 4 {
        0 => Arc::new(
            (0..rows)
                .map(|row| (row % 7 != 0).then_some(i64::try_from(row).unwrap_or_default()))
                .collect::<Int64Array>(),
        ) as ArrayRef,
        1 => Arc::new(
            (0..rows)
                .map(|row| (row % 7 != 0).then_some(row as i128 * 10001))
                .collect::<Decimal128Array>()
                .with_precision_and_scale(20, 4)?,
        ),
        2 => Arc::new(
            (0..rows)
                .map(|row| (row % 7 != 0).then_some("test-only ම"))
                .collect::<StringArray>(),
        ),
        _ => Arc::new(
            (0..rows)
                .map(|row| (row % 7 != 0).then_some(&b"test-only\0"[..]))
                .collect::<BinaryArray>(),
        ),
    })
}
fn invalid() -> DriverError {
    DriverError::Unsupported("test-only fixture shape".into())
}

/// Identical exclusive-row producer, capped below the total 64 MiB stream budget.
/// # Errors
/// Oversized fixture or invalid Arrow representation.
pub fn fixture_large(bytes: usize) -> Result<RecordBatch, DriverError> {
    if bytes > 63 * 1024 * 1024 {
        return Err(invalid());
    }
    let text = "x".repeat(bytes);
    let array = Arc::new(StringArray::from(vec![text.as_str()]));
    let schema = Arc::new(Schema::new(vec![Field::new(
        "large",
        DataType::Utf8,
        false,
    )]));
    Ok(RecordBatch::try_new(schema, vec![array])?)
}
