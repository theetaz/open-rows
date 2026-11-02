// SPDX-License-Identifier: Apache-2.0
//! [`crate::Group::MillionRows`]: one million rows through the streaming path.
//!
//! The sink keeps nothing, so resident memory measures what the driver
//! buffers. Each test runs in its own process under nextest, which makes the
//! process's resident size a clean measure. The result is about 208 MB; a
//! driver that buffered it would exceed [`MAX_GROWTH_KIB`] many times over.

use std::process::Command;
use std::time::{Duration, Instant};

use arrow_array::RecordBatch;
use arrow_array::cast::AsArray;
use arrow_array::types::{Int32Type, Int64Type};
use arrow_schema::{DataType, SchemaRef};
use openrows_driver_api::{BatchPolicy, ResultSink, SinkFlow};

use crate::dialect::million_rows;
use crate::{Failure, Target, ensure};

/// Rows in the fixture statement.
const ROWS: u64 = 1_000_000;

/// Characters of the text column.
const PAYLOAD: usize = 200;

/// Sum of `1..=ROWS`.
const SUM: i128 = 500_000_500_000;

/// Largest allowed resident growth while streaming.
const MAX_GROWTH_KIB: u64 = 64 * 1024;

/// Resident memory is sampled once per this many batches.
const SAMPLE_EVERY: u64 = 8;

/// Counts what arrives and drops it.
struct Tally {
    rows: u64,
    sum: i128,
    batches: u64,
    largest_batch: usize,
    first_batch: Option<usize>,
    results: u64,
    peak_kib: u64,
    problem: Option<String>,
}

impl Tally {
    fn add(&mut self, batch: &RecordBatch) -> Result<(), String> {
        let [numbers, payload] = batch.columns() else {
            return Err(format!("{} columns", batch.num_columns()));
        };
        self.sum += match numbers.data_type() {
            DataType::Int32 => numbers
                .as_primitive::<Int32Type>()
                .iter()
                .flatten()
                .map(i128::from)
                .sum::<i128>(),
            DataType::Int64 => numbers
                .as_primitive::<Int64Type>()
                .iter()
                .flatten()
                .map(i128::from)
                .sum::<i128>(),
            other => return Err(format!("n decoded as {other}")),
        };
        let texts = payload
            .as_string_opt::<i32>()
            .ok_or_else(|| format!("payload decoded as {}", payload.data_type()))?;
        if texts.iter().any(|text| text.map(str::len) != Some(PAYLOAD)) {
            return Err("a payload lost characters".to_owned());
        }
        self.rows += batch.num_rows() as u64;
        self.largest_batch = self.largest_batch.max(batch.num_rows());
        self.first_batch.get_or_insert(batch.num_rows());
        Ok(())
    }
}

impl ResultSink for Tally {
    fn begin_result(&mut self, _schema: SchemaRef) {
        self.results += 1;
    }

    fn push_batch(&mut self, batch: RecordBatch) -> SinkFlow {
        self.batches += 1;
        if let Err(problem) = self.add(&batch) {
            self.problem = Some(problem);
            return SinkFlow::Stop;
        }
        if self.batches.is_multiple_of(SAMPLE_EVERY) {
            match resident_kib() {
                Ok(now) => self.peak_kib = self.peak_kib.max(now),
                Err(problem) => {
                    self.problem = Some(problem);
                    return SinkFlow::Stop;
                }
            }
        }
        SinkFlow::Continue
    }
}

/// This process's resident size; `ps` needs no platform-specific code.
fn resident_kib() -> Result<u64, String> {
    let pid = std::process::id().to_string();
    let output = Command::new("ps")
        .args(["-o", "rss=", "-p", &pid])
        .output()
        .map_err(|error| format!("ps: {error}"))?;
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .map_err(|error| format!("ps printed no size: {error}"))
}

pub(crate) async fn run(target: &Target) -> Result<(), Failure> {
    let mut connection = target.connect().await?;
    let baseline = resident_kib().map_err(Failure::Setup)?;
    let mut tally = Tally {
        rows: 0,
        sum: 0,
        batches: 0,
        largest_batch: 0,
        first_batch: None,
        results: 0,
        peak_kib: baseline,
        problem: None,
    };
    let sql = million_rows(target.driver.dialect());
    let started = Instant::now();
    let summary = connection.execute(&sql, &mut tally).await?;
    connection.close().await?;
    if let Some(problem) = tally.problem.take() {
        return Err(Failure::Contract(problem));
    }
    ensure(
        tally.results == 1 && tally.rows == ROWS && summary.rows == ROWS,
        || {
            format!(
                "{} results, sink {} rows, summary {} rows, expected {ROWS}",
                tally.results, tally.rows, summary.rows
            )
        },
    )?;
    ensure(tally.sum == SUM && summary.batches == tally.batches, || {
        format!(
            "sum {} (expected {SUM}), summary {} batches, sink {}",
            tally.sum, summary.batches, tally.batches
        )
    })?;
    batches_follow_the_policy(&tally)?;
    let growth = tally.peak_kib.saturating_sub(baseline);
    report(&tally, growth, started.elapsed());
    ensure(growth <= MAX_GROWTH_KIB, || {
        format!("resident memory grew {growth} KiB, limit {MAX_GROWTH_KIB} KiB")
    })
}

/// The measurement for the acceptance report; visible with `--no-capture`.
#[allow(clippy::print_stderr, reason = "test diagnostics for the report")]
fn report(tally: &Tally, growth: u64, elapsed: Duration) {
    eprintln!(
        "[conformance] {} rows in {} batches, {elapsed:?}, resident growth {growth} KiB",
        tally.rows, tally.batches
    );
}

fn batches_follow_the_policy(tally: &Tally) -> Result<(), Failure> {
    let policy = BatchPolicy::default();
    let first = tally.first_batch.unwrap_or(0);
    ensure(first > 0 && first <= policy.first_batch_rows, || {
        format!(
            "first batch holds {first} rows (1..={})",
            policy.first_batch_rows
        )
    })?;
    let limit = policy.target_cells / 2;
    ensure(tally.largest_batch <= limit, || {
        format!("a batch holds {} rows, limit {limit}", tally.largest_batch)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resident_size_is_readable() -> Result<(), String> {
        assert!(resident_kib()? > 0);
        Ok(())
    }
}
