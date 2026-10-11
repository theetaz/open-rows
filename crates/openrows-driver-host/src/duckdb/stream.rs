// SPDX-License-Identifier: Apache-2.0
//! Fallible native chunk fetch; no materialized query result or panicking Arrow iterator.
use super::{bridge, error};
use duckdb::arrow::{array::RecordBatch, datatypes::DataType};
use openrows_driver_api::{BatchPolicy, DriverError, ExecSummary, ResultSink, SinkFlow};

pub(super) fn run(
    native: &duckdb::Connection,
    sql: &str,
    params: &[Option<String>],
    sink: &mut dyn ResultSink,
) -> Result<(ExecSummary, bool), DriverError> {
    let mut statement = native.prepare(sql).map_err(error::map)?;
    if statement.parameter_count() != params.len() {
        return Err(DriverError::Server {
            code: "07001".into(),
            message: format!(
                "DuckDB statement requires {} parameters, received {}",
                statement.parameter_count(),
                params.len()
            ),
        });
    }
    // Dropping this wrapper leaves the result on the statement. `step()` is fallible.
    let schema = statement
        .stream_arrow(duckdb::params_from_iter(
            params.iter().map(|v| v.as_deref()),
        ))
        .map_err(error::map)?
        .get_schema();
    let types = (0..schema.fields().len())
        .map(|index| format!("{:?}", statement.column_logical_type(index)))
        .collect::<Vec<_>>();
    if is_count(sql, &schema) {
        return affected(&statement, sql).map(|summary| (summary, false));
    }
    let empty = bridge::convert(&RecordBatch::new_empty(schema.clone()), &types)?;
    sink.begin_result(empty.schema());
    let mut summary = ExecSummary::default();
    let mut stopped = false;
    while let Some(array) = statement.step().map_err(error::map)? {
        let batch = RecordBatch::from(&array);
        if !publish(&batch, &types, sink, &mut summary)? {
            stopped = true;
            break;
        }
    }
    summary.command_tag = format!("SELECT {}", summary.rows);
    Ok((summary, stopped))
}
fn is_count(sql: &str, schema: &duckdb::arrow::datatypes::Schema) -> bool {
    let kind = openrows_sql::classify(sql, openrows_sql::Dialect::Duckdb).kind;
    schema.fields().len() == 1
        && schema.field(0).name() == "Count"
        && schema.field(0).data_type() == &DataType::Int64
        && matches!(
            kind,
            openrows_sql::StatementKind::Write | openrows_sql::StatementKind::Ddl
        )
        && !openrows_sql::duckdb::has_returning(sql)
}
fn affected(statement: &duckdb::Statement<'_>, sql: &str) -> Result<ExecSummary, DriverError> {
    let mut count = 0_i64;
    while let Some(array) = statement.step().map_err(error::map)? {
        let value = array
            .column(0)
            .as_any()
            .downcast_ref::<duckdb::arrow::array::Int64Array>()
            .ok_or_else(|| {
                DriverError::Protocol("DuckDB affected-row count is not an integer".into())
            })?;
        count += value.values().iter().sum::<i64>();
    }
    let verb = sql
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    Ok(ExecSummary {
        command_tag: format!("{verb} {count}"),
        ..ExecSummary::default()
    })
}
fn publish(
    batch: &RecordBatch,
    types: &[String],
    sink: &mut dyn ResultSink,
    summary: &mut ExecSummary,
) -> Result<bool, DriverError> {
    let policy = BatchPolicy::default();
    let mut offset = 0;
    while offset < batch.num_rows() {
        let rows = policy
            .row_limit(batch.num_columns(), summary.batches == 0)
            .min(batch.num_rows() - offset);
        let received = std::time::Instant::now();
        let (part, rows) = convert_part(batch, types, offset, rows)?;
        summary.first_batch_latency = summary.first_batch_latency.or(Some(received.elapsed()));
        for part in openrows_host_protocol::split_batch(&part).map_err(limit)? {
            let part = part.map_err(limit)?;
            summary.rows += part.num_rows() as u64;
            summary.batches += 1;
            if sink.push_batch(part) == SinkFlow::Stop {
                return Ok(false);
            }
        }
        offset += rows;
    }
    Ok(true)
}
fn convert_part(
    batch: &RecordBatch,
    types: &[String],
    offset: usize,
    mut rows: usize,
) -> Result<(arrow_array::RecordBatch, usize), DriverError> {
    loop {
        match bridge::convert(&batch.slice(offset, rows), types) {
            Ok(part) => return Ok((part, rows)),
            Err(DriverError::Unsupported(_)) if rows > 1 => rows = rows.div_ceil(2),
            Err(error) => return Err(error),
        }
    }
}
fn limit(error: openrows_host_protocol::ProtocolError) -> DriverError {
    DriverError::Unsupported(format!(
        "DuckDB row exceeds 64 MiB encoded or decoded ceiling: {error}"
    ))
}
