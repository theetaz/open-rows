// SPDX-License-Identifier: Apache-2.0
//! Shared helpers of the engine import tests: write a file, run a whole
//! import (inspect, feed, load) on a connection and count matching rows.
#![allow(
    dead_code,
    reason = "each engine test uses a different subset of the helpers"
)]
pub mod scenarios;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use openrows_driver_api::{Connection, ResultSink, SinkFlow};
use openrows_import::{
    BadRows, CancellationToken, ColumnSource, ConflictPolicy, Control, FeedSource, LoadPlan,
    MappedColumn, Mapper, Outcome, RunMode, SampleLimits, Summary, TableName, TransactionMode,
    TypeCategory, inspect, load, spawn_feed,
};
use openrows_sql::Dialect;

pub type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// Write `text` to `name` in `dir`.
pub fn write(dir: &tempfile::TempDir, name: &str, text: &str) -> TestResult<PathBuf> {
    let path = dir.path().join(name);
    std::fs::write(&path, text)?;
    Ok(path)
}

/// Source column `i` into target column `names[i].0`, checked as `names[i].1`.
pub fn identity(names: &[(&str, TypeCategory)]) -> Vec<MappedColumn> {
    names
        .iter()
        .enumerate()
        .map(|(index, (name, category))| MappedColumn {
            target: (*name).to_owned(),
            source: ColumnSource::Column(index),
            category: *category,
        })
        .collect()
}

/// A plain single-transaction append into `schema.table`.
pub fn plan(dialect: Dialect, (schema, table): (&str, &str), mapping: &[MappedColumn]) -> LoadPlan {
    LoadPlan {
        dialect,
        table: TableName {
            schema: schema.to_owned(),
            table: table.to_owned(),
        },
        columns: mapping.iter().map(|column| column.target.clone()).collect(),
        key: Vec::new(),
        conflict: ConflictPolicy::Error,
        create: None,
        truncate: false,
        transaction: TransactionMode::Single,
        bad_rows: BadRows::Skip,
        mode: RunMode::Import,
    }
}

/// Inspect `path`, then feed and load it with `batch_rows` rows per batch.
pub async fn import(
    connection: &mut dyn Connection,
    path: &Path,
    (mapping, plan): (Vec<MappedColumn>, &LoadPlan),
    batch_rows: usize,
) -> TestResult<Outcome> {
    let found = inspect(
        path,
        None,
        SampleLimits {
            preview: 5,
            sample: 50,
        },
    )?;
    let columns: Vec<String> = found.columns.iter().map(|c| c.name.clone()).collect();
    let mapper = Mapper::new(mapping, plan.dialect, columns.len())?;
    let feed = spawn_feed(
        FeedSource {
            path: path.to_path_buf(),
            options: found.options,
            columns,
            mapper,
            batch_rows,
        },
        Arc::default(),
    );
    let control = Control {
        cancel: CancellationToken::new(),
        progress: Box::new(|_| {}),
    };
    Ok(load(connection, plan, feed, control).await)
}

/// The summary of a finished import, or the failure as an error.
pub fn finished(outcome: Outcome) -> TestResult<Summary> {
    match outcome {
        Outcome::Finished(summary) => Ok(summary),
        other => Err(format!("import did not finish: {other:?}").into()),
    }
}

/// Counts the rows a statement returns.
struct Count;

impl ResultSink for Count {
    fn begin_result(&mut self, _schema: SchemaRef) {}

    fn push_batch(&mut self, _batch: RecordBatch) -> SinkFlow {
        SinkFlow::Continue
    }
}

/// Rows returned by `sql`.
pub async fn count(connection: &mut dyn Connection, sql: &str) -> TestResult<u64> {
    Ok(connection.execute(sql, &mut Count).await?.rows)
}

/// Run `sql`, ignoring rows.
pub async fn run(connection: &mut dyn Connection, sql: &str) -> TestResult {
    connection.execute(sql, &mut Count).await?;
    Ok(())
}
