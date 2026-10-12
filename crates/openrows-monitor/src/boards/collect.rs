// SPDX-License-Identifier: Apache-2.0
//! Bounded collection of a widget's first result set into typed text cells.
use arrow_array::{Array, RecordBatch};
use arrow_cast::display::{ArrayFormatter, FormatOptions};
use arrow_schema::{DataType, Schema};
use serde::Serialize;

use super::model::WidgetLimits;

const MAX_COLUMNS: usize = 64;
const MAX_CELL: usize = 4_096;

/// How a column's values can be mapped onto a chart axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ipc", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ColumnKind {
    /// Integer, float or decimal.
    Number,
    /// Date, time or timestamp, as ISO 8601 text.
    Time,
    /// Anything else, as display text.
    Text,
}

/// A result column.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ipc", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BoardColumn {
    /// Column name.
    pub name: String,
    /// Value kind.
    pub kind: ColumnKind,
}

/// Which bound stopped the result early.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ipc", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum Truncation {
    /// The row limit.
    Rows,
    /// The byte limit.
    Bytes,
}

/// A bounded widget result; `None` cells are SQL NULL.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ipc", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BoardResult {
    /// Columns of the first result set, at most 64.
    pub columns: Vec<BoardColumn>,
    /// Rows as display text.
    pub rows: Vec<Vec<Option<String>>>,
    /// Set when a limit stopped the stream.
    pub truncated: Option<Truncation>,
}

/// Collects the first result set within a widget's row and byte limits.
#[derive(Debug)]
pub struct Collector {
    limits: WidgetLimits,
    result: BoardResult,
    bytes: usize,
    begun: bool,
}

impl Collector {
    /// An empty collector bounded by `limits`.
    #[must_use]
    pub fn new(limits: WidgetLimits) -> Self {
        Self {
            limits,
            result: BoardResult::default(),
            bytes: 0,
            begun: false,
        }
    }

    /// A result set starts; only the first one is kept.
    pub fn begin(&mut self, schema: &Schema) {
        if self.begun {
            return;
        }
        self.begun = true;
        let fields = schema.fields().iter().take(MAX_COLUMNS);
        self.result.columns = fields
            .map(|field| BoardColumn {
                name: field.name().clone(),
                kind: kind(field.data_type()),
            })
            .collect();
    }

    /// Keep `batch`'s rows; `false` asks the driver to stop.
    pub fn push(&mut self, batch: &RecordBatch) -> bool {
        if self.result.truncated.is_some() {
            return false;
        }
        let options = FormatOptions::default();
        let formatters: Vec<Option<ArrayFormatter<'_>>> = batch
            .columns()
            .iter()
            .take(MAX_COLUMNS)
            .map(|array| ArrayFormatter::try_new(array.as_ref(), &options).ok())
            .collect();
        for row in 0..batch.num_rows() {
            if let Some(reason) = self.full() {
                self.result.truncated = Some(reason);
                return false;
            }
            let cells = cells(batch, &formatters, row);
            self.bytes += cells.iter().flatten().map(String::len).sum::<usize>();
            self.result.rows.push(cells);
        }
        true
    }

    fn full(&self) -> Option<Truncation> {
        if self.result.rows.len() >= self.limits.max_rows as usize {
            Some(Truncation::Rows)
        } else if self.bytes >= self.limits.max_bytes as usize {
            Some(Truncation::Bytes)
        } else {
            None
        }
    }

    /// The collected result.
    #[must_use]
    pub fn finish(self) -> BoardResult {
        self.result
    }
}

fn cells(
    batch: &RecordBatch,
    formatters: &[Option<ArrayFormatter<'_>>],
    row: usize,
) -> Vec<Option<String>> {
    batch
        .columns()
        .iter()
        .zip(formatters)
        .map(|(array, formatter)| {
            if array.is_null(row) {
                return None;
            }
            let text = formatter
                .as_ref()
                .map_or_else(String::new, |f| f.value(row).to_string());
            Some(clip(text))
        })
        .collect()
}

fn clip(mut text: String) -> String {
    if text.len() > MAX_CELL {
        let mut end = MAX_CELL;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
    }
    text
}

fn kind(data_type: &DataType) -> ColumnKind {
    match data_type {
        DataType::Int8
        | DataType::Int16
        | DataType::Int32
        | DataType::Int64
        | DataType::UInt8
        | DataType::UInt16
        | DataType::UInt32
        | DataType::UInt64
        | DataType::Float16
        | DataType::Float32
        | DataType::Float64
        | DataType::Decimal32(..)
        | DataType::Decimal64(..)
        | DataType::Decimal128(..)
        | DataType::Decimal256(..) => ColumnKind::Number,
        DataType::Date32
        | DataType::Date64
        | DataType::Timestamp(..)
        | DataType::Time32(_)
        | DataType::Time64(_) => ColumnKind::Time,
        _ => ColumnKind::Text,
    }
}
