// SPDX-License-Identifier: Apache-2.0
//! The SDK owns cursor server selection, getMore and addressed drop cleanup.
use crate::{command, error};
use arrow_schema::SchemaRef;
use futures_core::Stream;
use mongodb::{
    Database,
    bson::{Bson, Document},
    raw_batch_cursor::RawBatch,
};
use openrows_driver_api::{DriverError, ExecSummary, ResultSink, SinkFlow};
use openrows_results::documents;
use serde_json::Value;
use std::{future::poll_fn, pin::Pin, sync::Arc};

pub(crate) async fn run(
    database: &Database,
    command: Document,
    (token, table, count): (&str, bool, bool),
    sink: &mut dyn ResultSink,
) -> Result<ExecSummary, DriverError> {
    let mut cursor = database
        .run_cursor_command(command)
        .batch_size(128)
        .comment(token.to_owned())
        .batch()
        .await
        .map_err(error::mongo)?;
    let mut schema = None;
    let mut summary = ExecSummary::default();
    while let Some(batch) = poll_fn(|context| Pin::new(&mut cursor).poll_next(context)).await {
        let batch = batch.map_err(error::mongo)?;
        command::check_raw_reply(batch.as_raw_document())?;
        let rows = values(&batch, count && summary.batches == 0)?;
        if emit((&mut schema, &mut summary), &rows, table, sink)? == SinkFlow::Stop {
            break;
        }
    }
    Ok(summary)
}

fn values(batch: &RawBatch, empty_count: bool) -> Result<Vec<Value>, DriverError> {
    let mut values = batch
        .doc_slices()
        .map_err(error::mongo)?
        .into_iter()
        .map(|value| {
            let value = value.map_err(|_| invalid())?;
            let document = Document::try_from(value.as_document().ok_or_else(invalid)?)
                .map_err(|_| invalid())?;
            Ok(Bson::Document(document).into_canonical_extjson())
        })
        .collect::<Result<Vec<_>, DriverError>>()?;
    if empty_count && values.is_empty() {
        let cursor = batch
            .as_raw_document()
            .get_document("cursor")
            .map_err(|_| invalid())?;
        if cursor.get_i64("id").map_err(|_| invalid())? == 0 {
            values.push(serde_json::json!({"count":{"$numberLong":"0"}}));
        }
    }
    Ok(values)
}

fn emit(
    (schema, summary): (&mut Option<SchemaRef>, &mut ExecSummary),
    rows: &[Value],
    table: bool,
    sink: &mut dyn ResultSink,
) -> Result<SinkFlow, DriverError> {
    let schema = schema
        .get_or_insert_with(|| result_schema(rows, table))
        .clone();
    if summary.batches == 0 {
        sink.begin_result(schema.clone());
    }
    let batch = documents::batch(schema, rows)?;
    summary.rows += u64::try_from(batch.num_rows()).unwrap_or(u64::MAX);
    summary.batches += 1;
    Ok(sink.push_batch(batch))
}
fn result_schema(rows: &[Value], table: bool) -> SchemaRef {
    let inferred = documents::schema(rows);
    if !table {
        return inferred;
    }
    Arc::new(arrow_schema::Schema::new(
        inferred
            .fields()
            .iter()
            .filter(|field| matches!(field.name().as_str(), "_id" | "__document"))
            .cloned()
            .collect::<Vec<_>>(),
    ))
}
fn invalid() -> DriverError {
    DriverError::Protocol("invalid MongoDB cursor response".into())
}
