// SPDX-License-Identifier: Apache-2.0
//! One statement on the connection thread: bind text parameters, step rows
//! into batches and hand them to the async side, which feeds the sink.
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use openrows_driver_api::{BatchPolicy, DriverError, ExecSummary};
use rusqlite::{Connection, Statement};
use tokio::sync::mpsc::Sender;

use crate::decode::{BatchBuffer, Cell, ColumnSpec};
use crate::error::driver_error;

/// What the connection thread reports while a statement runs.
pub(crate) enum Event {
    /// A result (or a continuation with a changed schema) starts.
    Begin(SchemaRef),
    /// The next batch of the current result.
    Batch(RecordBatch),
}

/// A statement to run, owned so it can move to the connection thread.
pub(crate) struct Job {
    pub(crate) sql: String,
    pub(crate) params: Vec<Option<String>>,
    pub(crate) policy: BatchPolicy,
}

/// Run `job`, sending results over `events` until done or `stop` is set.
pub(crate) fn run(
    connection: &Connection,
    job: &Job,
    events: &Sender<Event>,
    stop: &AtomicBool,
) -> Result<ExecSummary, DriverError> {
    let mut statement = connection.prepare(&job.sql).map_err(|e| driver_error(&e))?;
    bind(&mut statement, &job.params)?;
    if statement.column_count() == 0 {
        let changes = statement.raw_execute().map_err(|e| driver_error(&e))?;
        return Ok(ExecSummary {
            command_tag: tag(&job.sql, Some(changes as u64)),
            ..ExecSummary::default()
        });
    }
    let columns: Arc<[ColumnSpec]> = statement
        .columns()
        .iter()
        .map(|column| ColumnSpec {
            name: column.name().to_owned(),
            declared: column.decl_type().map(str::to_owned),
        })
        .collect();
    let mut publisher = Publisher {
        events,
        stop,
        published: None,
        summary: ExecSummary::default(),
    };
    step_rows(
        &mut statement,
        Arc::clone(&columns),
        job.policy,
        &mut publisher,
    )?;
    let rows = publisher.summary.rows;
    publisher.summary.command_tag = tag(&job.sql, Some(rows));
    Ok(publisher.summary)
}

fn bind(statement: &mut Statement<'_>, params: &[Option<String>]) -> Result<(), DriverError> {
    if statement.parameter_count() != params.len() {
        return Err(DriverError::Server {
            code: "07001".to_owned(),
            message: format!(
                "the statement has {} parameters but {} values were given",
                statement.parameter_count(),
                params.len()
            ),
        });
    }
    for (index, value) in params.iter().enumerate() {
        statement
            .raw_bind_parameter(index + 1, value.as_deref())
            .map_err(|e| driver_error(&e))?;
    }
    Ok(())
}

fn step_rows(
    statement: &mut Statement<'_>,
    columns: Arc<[ColumnSpec]>,
    policy: BatchPolicy,
    publisher: &mut Publisher<'_>,
) -> Result<(), DriverError> {
    let width = columns.len();
    let mut buffer = BatchBuffer::new(columns);
    let mut rows = statement.raw_query();
    while let Some(row) = rows.next().map_err(|e| driver_error(&e))? {
        let cells = (0..width)
            .map(|index| row.get_ref(index).map(Cell::from_ref))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| driver_error(&e))?;
        buffer.push(cells);
        let limit = policy.row_limit(width, publisher.summary.batches == 0);
        if policy.should_flush(buffer.rows(), buffer.bytes(), limit)
            && !publisher.flush(&mut buffer)?
        {
            return Ok(());
        }
    }
    if buffer.rows() > 0 || publisher.published.is_none() {
        publisher.flush(&mut buffer)?;
    }
    Ok(())
}

/// Sends batches, starting a new result whenever the schema changes.
struct Publisher<'a> {
    events: &'a Sender<Event>,
    stop: &'a AtomicBool,
    published: Option<SchemaRef>,
    summary: ExecSummary,
}

impl Publisher<'_> {
    /// Send the buffered rows; `false` when the consumer wants no more.
    fn flush(&mut self, buffer: &mut BatchBuffer) -> Result<bool, DriverError> {
        let batch = buffer.finish()?;
        let schema = batch.schema();
        if self.published.as_ref() != Some(&schema) {
            self.published = Some(Arc::clone(&schema));
            if self.events.blocking_send(Event::Begin(schema)).is_err() {
                return Ok(false);
            }
        }
        if batch.num_rows() == 0 {
            return Ok(true);
        }
        self.summary.rows += batch.num_rows() as u64;
        self.summary.batches += 1;
        let sent = self.events.blocking_send(Event::Batch(batch)).is_ok();
        Ok(sent && !self.stop.load(Ordering::SeqCst))
    }
}

/// A `PostgreSQL`-style command tag, e.g. `SELECT 3`, `UPDATE 1` or `CREATE`.
fn tag(sql: &str, count: Option<u64>) -> String {
    let verb = sql
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let counted = matches!(
        verb.as_str(),
        "SELECT" | "VALUES" | "WITH" | "INSERT" | "UPDATE" | "DELETE" | "REPLACE"
    );
    match count {
        Some(count) if counted => format!("{verb} {count}"),
        _ => verb,
    }
}

#[cfg(test)]
mod tests {
    use super::tag;

    #[test]
    fn tags_count_rows_only_for_data_statements() {
        assert_eq!(tag("select 1", Some(1)), "SELECT 1");
        assert_eq!(tag("UPDATE t SET a = 1", Some(3)), "UPDATE 3");
        assert_eq!(tag("CREATE TABLE t (a)", Some(7)), "CREATE");
        assert_eq!(tag("", None), "");
    }
}
