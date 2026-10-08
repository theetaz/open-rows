// SPDX-License-Identifier: Apache-2.0
//! `SQLite` values into Arrow batches.
//!
//! `SQLite` types values, not columns, so each batch picks a column type
//! from the declared type's affinity and the values it holds: only integers
//! → `Int64`, only reals (or integers in a `REAL` column) → `Float64`, only
//! text → `Utf8`, only blobs → `Binary`. Any other mix keeps every value
//! exact as tagged text (`openrows.text_fallback`), as the `PostgreSQL`
//! driver does for values Arrow cannot hold (docs/05 §12).
use std::collections::HashMap;
use std::sync::Arc;

use arrow_array::{ArrayRef, BinaryArray, Float64Array, Int64Array, RecordBatch, StringArray};
use arrow_schema::{ArrowError, DataType, Field, Schema};
use openrows_driver_api::metadata::{SOURCE_TYPE, TEXT_FALLBACK};
use rusqlite::types::ValueRef;

/// One owned value.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Cell {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl Cell {
    pub(crate) fn from_ref(value: ValueRef<'_>) -> Self {
        match value {
            ValueRef::Null => Self::Null,
            ValueRef::Integer(integer) => Self::Integer(integer),
            ValueRef::Real(real) => Self::Real(real),
            ValueRef::Text(bytes) => Self::Text(String::from_utf8_lossy(bytes).into_owned()),
            ValueRef::Blob(bytes) => Self::Blob(bytes.to_vec()),
        }
    }

    /// Approximate decoded size, for the batch byte budget.
    pub(crate) fn size(&self) -> usize {
        match self {
            Self::Null => 1,
            Self::Integer(_) | Self::Real(_) => 8,
            Self::Text(text) => text.len(),
            Self::Blob(bytes) => bytes.len(),
        }
    }

    /// The value's text form (blobs as `x'…'` hex); `None` for NULL.
    fn text(&self) -> Option<String> {
        match self {
            Self::Null => None,
            Self::Integer(integer) => Some(integer.to_string()),
            Self::Real(real) => Some(real.to_string()),
            Self::Text(text) => Some(text.clone()),
            Self::Blob(bytes) => Some(format!("x'{}'", hex(bytes))),
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(char::from(DIGITS[usize::from(byte >> 4)]));
        text.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    text
}

/// Type affinity of a declared column type (`SQLite` §3.1 rules).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Affinity {
    Integer,
    Text,
    Blob,
    Real,
    Numeric,
}

impl Affinity {
    pub(crate) fn of(declared: Option<&str>) -> Self {
        let upper = declared.unwrap_or_default().to_ascii_uppercase();
        if upper.contains("INT") {
            Self::Integer
        } else if ["CHAR", "CLOB", "TEXT"]
            .iter()
            .any(|word| upper.contains(word))
        {
            Self::Text
        } else if upper.is_empty() || upper.contains("BLOB") {
            Self::Blob
        } else if ["REAL", "FLOA", "DOUB"]
            .iter()
            .any(|word| upper.contains(word))
        {
            Self::Real
        } else {
            Self::Numeric
        }
    }
}

/// A result column: name and declared type (`None` for expressions).
#[derive(Clone, Debug)]
pub(crate) struct ColumnSpec {
    pub(crate) name: String,
    pub(crate) declared: Option<String>,
}

/// Which storage classes a column's values use in one batch, as
/// `[integer, real, text, blob]`.
struct Seen([bool; 4]);

impl Seen {
    fn of<'a>(cells: impl Iterator<Item = &'a Cell>) -> Self {
        let mut seen = [false; 4];
        for cell in cells {
            let class = match cell {
                Cell::Null => continue,
                Cell::Integer(_) => 0,
                Cell::Real(_) => 1,
                Cell::Text(_) => 2,
                Cell::Blob(_) => 3,
            };
            seen[class] = true;
        }
        Self(seen)
    }
}

/// How one column is written into Arrow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Plan {
    Integer,
    Real,
    Text,
    Blob,
    /// Mixed storage classes: every value as exact text.
    Fallback,
}

fn plan(affinity: Affinity, seen: &Seen) -> Plan {
    let [integer, real, text, blob] = seen.0;
    match (integer, real, text, blob) {
        (false, false, false, false) => match affinity {
            Affinity::Integer => Plan::Integer,
            Affinity::Real => Plan::Real,
            Affinity::Blob => Plan::Blob,
            Affinity::Text | Affinity::Numeric => Plan::Text,
        },
        (true, false, false, false) => Plan::Integer,
        (_, true, false, false) if !integer || affinity == Affinity::Real => Plan::Real,
        (false, false, true, false) => Plan::Text,
        (false, false, false, true) => Plan::Blob,
        _ => Plan::Fallback,
    }
}

/// Rows of one batch, column-major on finish.
pub(crate) struct BatchBuffer {
    columns: Arc<[ColumnSpec]>,
    rows: Vec<Vec<Cell>>,
    bytes: usize,
}

impl BatchBuffer {
    pub(crate) fn new(columns: Arc<[ColumnSpec]>) -> Self {
        Self {
            columns,
            rows: Vec::new(),
            bytes: 0,
        }
    }

    pub(crate) fn push(&mut self, row: Vec<Cell>) {
        self.bytes += row.iter().map(Cell::size).sum::<usize>();
        self.rows.push(row);
    }

    pub(crate) fn rows(&self) -> usize {
        self.rows.len()
    }

    pub(crate) fn bytes(&self) -> usize {
        self.bytes
    }

    /// Build the batch and empty the buffer.
    pub(crate) fn finish(&mut self) -> Result<RecordBatch, ArrowError> {
        let rows = std::mem::take(&mut self.rows);
        self.bytes = 0;
        let mut fields = Vec::with_capacity(self.columns.len());
        let mut arrays = Vec::with_capacity(self.columns.len());
        for (index, spec) in self.columns.iter().enumerate() {
            let cells: Vec<&Cell> = rows.iter().filter_map(|row| row.get(index)).collect();
            let affinity = Affinity::of(spec.declared.as_deref());
            let plan = plan(affinity, &Seen::of(cells.iter().copied()));
            fields.push(field(spec, plan, &cells));
            arrays.push(array(plan, &cells));
        }
        let schema = Arc::new(Schema::new(fields));
        if rows.is_empty() {
            return Ok(RecordBatch::new_empty(schema));
        }
        RecordBatch::try_new(schema, arrays)
    }
}

fn field(spec: &ColumnSpec, plan: Plan, cells: &[&Cell]) -> Field {
    let data_type = match plan {
        Plan::Integer => DataType::Int64,
        Plan::Real => DataType::Float64,
        Plan::Text | Plan::Fallback => DataType::Utf8,
        Plan::Blob => DataType::Binary,
    };
    let source = spec.declared.clone().unwrap_or_else(|| storage_name(cells));
    let mut metadata = HashMap::from([(SOURCE_TYPE.to_owned(), source)]);
    if plan == Plan::Fallback {
        metadata.insert(TEXT_FALLBACK.to_owned(), "true".to_owned());
    }
    Field::new(&spec.name, data_type, true).with_metadata(metadata)
}

/// Storage class of an expression column's first non-NULL value.
fn storage_name(cells: &[&Cell]) -> String {
    let name = cells.iter().find_map(|cell| match cell {
        Cell::Null => None,
        Cell::Integer(_) => Some("INTEGER"),
        Cell::Real(_) => Some("REAL"),
        Cell::Text(_) => Some("TEXT"),
        Cell::Blob(_) => Some("BLOB"),
    });
    name.unwrap_or("NULL").to_owned()
}

#[expect(
    clippy::cast_precision_loss,
    reason = "integers in a REAL column are stored as REAL by SQLite itself"
)]
fn array(plan: Plan, cells: &[&Cell]) -> ArrayRef {
    match plan {
        Plan::Integer => Arc::new(collect::<Int64Array, _>(cells.iter().map(
            |cell| match cell {
                Cell::Integer(value) => Some(*value),
                _ => None,
            },
        ))),
        Plan::Real => Arc::new(collect::<Float64Array, _>(cells.iter().map(
            |cell| match cell {
                Cell::Real(value) => Some(*value),
                Cell::Integer(value) => Some(*value as f64),
                _ => None,
            },
        ))),
        Plan::Blob => Arc::new(collect::<BinaryArray, _>(cells.iter().map(
            |cell| match cell {
                Cell::Blob(bytes) => Some(bytes.as_slice()),
                _ => None,
            },
        ))),
        Plan::Text | Plan::Fallback => Arc::new(collect::<StringArray, _>(
            cells.iter().map(|cell| cell.text()),
        )),
    }
}

/// Collect an Arrow array from optional values.
fn collect<A: FromIterator<T>, T>(values: impl Iterator<Item = T>) -> A {
    values.collect()
}

#[cfg(test)]
mod tests;
