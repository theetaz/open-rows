// SPDX-License-Identifier: Apache-2.0
//! Result sets into Arrow batches for the result pipeline (docs/04 §5, docs/05 §2).
//!
//! | `MySQL` type | Arrow | Field metadata |
//! | --- | --- | --- |
//! | `TINYINT` … `INT`, signed `BIGINT` | `Int64` | |
//! | unsigned `BIGINT` | `UInt64` | |
//! | `YEAR` | `Int16` | |
//! | `FLOAT` / `DOUBLE` | `Float32` / `Float64` | |
//! | `DECIMAL(p, s)`, p ≤ 38 | `Decimal128(p, s)` | |
//! | `DECIMAL(p, s)`, p > 38 | `Utf8`, exact text | `openrows.decimal` |
//! | `BIT(n)` | `UInt64` | |
//! | `JSON` | `Utf8` | `openrows.json` |
//! | `DATE` | `Date32` | |
//! | `DATETIME(fsp)`, `TIMESTAMP(fsp)` | `Timestamp(µs)`, no zone | `openrows.fraction_digits` |
//! | `TIME(fsp)` | `Time64(µs)` | `openrows.fraction_digits` |
//! | text types, `ENUM`, `SET` | `Utf8` | |
//! | binary strings, `BLOB`, `GEOMETRY` | `Binary` | |
//!
//! `TIMESTAMP` values are the session time zone's wall-clock time, exactly as
//! the server prints them (no zone is attached: the session zone may be
//! `SYSTEM` with daylight saving, so a UTC instant could not be recovered
//! exactly). Zero and partial dates (`0000-00-00`), negative `TIME` or `TIME`
//! of 24 hours and more, and decimals outside the column's precision have no
//! exact Arrow form: the first such value switches its column to tagged text
//! (`openrows.text_fallback`, the server's text) for the rest of the result,
//! as the `PostgreSQL` driver does. Every field carries `openrows.source_type`.
mod builder;
mod decimal;
mod fallback;
mod temporal;

use std::collections::HashMap;
use std::sync::Arc;

use arrow_schema::{DataType, Field, Schema, SchemaRef, TimeUnit};
use mysql_common::constants::{ColumnFlags, ColumnType};
use mysql_common::packets::Column;
use openrows_driver_api::metadata::{
    DECIMAL_EXTENSION, EXTENSION_NAME, FRACTION_DIGITS, JSON_EXTENSION, SOURCE_TYPE, TEXT_FALLBACK,
};

use crate::values::unsigned;

pub(crate) use builder::BatchBuilder;

/// The `binary` character set: bytes, not text.
const BINARY_CHARSET: u16 = 63;
/// Largest precision `Decimal128` holds.
const MAX_DECIMAL128: u8 = 38;
/// Largest fractional-second precision; `decimals` above it means "not fixed".
const MAX_FSP: u8 = 6;

/// How one column's text values become Arrow values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Kind {
    Int,
    UInt,
    Year,
    Float,
    Double,
    Bit,
    Text,
    Bytes,
    Date,
    /// `DATETIME` and `TIMESTAMP`; `fsp` is `None` when not fixed.
    DateTime {
        fsp: Option<u8>,
    },
    Time {
        fsp: Option<u8>,
    },
    Decimal {
        precision: u8,
        scale: i8,
    },
}

/// Why one cell could not be decoded.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CellError {
    /// The text is not a value of the column's type.
    Malformed(String),
    /// No exact Arrow value; the column switches to tagged text.
    Unrepresentable(&'static str),
}

impl std::fmt::Display for CellError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed(reason) => f.write_str(reason),
            Self::Unrepresentable(what) => write!(f, "{what} value has no exact Arrow form"),
        }
    }
}

/// Kind, source type name and extension of `column`.
pub(crate) fn describe(column: &Column) -> (Kind, &'static str, Option<&'static str>) {
    use ColumnType as T;
    let binary = column.character_set() == BINARY_CHARSET;
    let flags = column.flags();
    let fsp = (column.decimals() <= MAX_FSP).then_some(column.decimals());
    match column.column_type() {
        T::MYSQL_TYPE_TINY => (Kind::Int, "tinyint", None),
        T::MYSQL_TYPE_SHORT => (Kind::Int, "smallint", None),
        T::MYSQL_TYPE_INT24 => (Kind::Int, "mediumint", None),
        T::MYSQL_TYPE_LONG => (Kind::Int, "int", None),
        T::MYSQL_TYPE_YEAR => (Kind::Year, "year", None),
        T::MYSQL_TYPE_LONGLONG if unsigned(column) => (Kind::UInt, "bigint unsigned", None),
        T::MYSQL_TYPE_LONGLONG => (Kind::Int, "bigint", None),
        T::MYSQL_TYPE_FLOAT => (Kind::Float, "float", None),
        T::MYSQL_TYPE_DOUBLE => (Kind::Double, "double", None),
        T::MYSQL_TYPE_DECIMAL | T::MYSQL_TYPE_NEWDECIMAL => decimal_kind(column),
        T::MYSQL_TYPE_BIT => (Kind::Bit, "bit", None),
        T::MYSQL_TYPE_JSON => (Kind::Text, "json", Some(JSON_EXTENSION)),
        T::MYSQL_TYPE_DATE | T::MYSQL_TYPE_NEWDATE => (Kind::Date, "date", None),
        T::MYSQL_TYPE_TIME | T::MYSQL_TYPE_TIME2 => (Kind::Time { fsp }, "time", None),
        T::MYSQL_TYPE_DATETIME | T::MYSQL_TYPE_DATETIME2 => {
            (Kind::DateTime { fsp }, "datetime", None)
        }
        T::MYSQL_TYPE_TIMESTAMP | T::MYSQL_TYPE_TIMESTAMP2 => {
            (Kind::DateTime { fsp }, "timestamp", None)
        }
        T::MYSQL_TYPE_GEOMETRY => (Kind::Bytes, "geometry", None),
        T::MYSQL_TYPE_VECTOR => (Kind::Bytes, "vector", None),
        _ if flags.contains(ColumnFlags::ENUM_FLAG) => (Kind::Text, "enum", None),
        _ if flags.contains(ColumnFlags::SET_FLAG) => (Kind::Text, "set", None),
        T::MYSQL_TYPE_TINY_BLOB
        | T::MYSQL_TYPE_MEDIUM_BLOB
        | T::MYSQL_TYPE_LONG_BLOB
        | T::MYSQL_TYPE_BLOB => blob(binary, "blob", "text"),
        T::MYSQL_TYPE_STRING => blob(binary, "binary", "char"),
        T::MYSQL_TYPE_VAR_STRING | T::MYSQL_TYPE_VARCHAR => blob(binary, "varbinary", "varchar"),
        _ => (Kind::Text, "unknown", None),
    }
}

/// `DECIMAL(p, s)` from the column's display length: the server sends
/// `p + (s > 0 ? 1 : 0) + (unsigned ? 0 : 1)` (point and sign).
fn decimal_kind(column: &Column) -> (Kind, &'static str, Option<&'static str>) {
    let scale = column.decimals();
    let extra = u32::from(scale > 0) + u32::from(!unsigned(column));
    let precision = column.column_length().checked_sub(extra);
    match (
        precision.and_then(|p| u8::try_from(p).ok()),
        i8::try_from(scale),
    ) {
        (Some(precision), Ok(scale))
            if precision > 0
                && precision <= MAX_DECIMAL128
                && scale.unsigned_abs() <= precision =>
        {
            (Kind::Decimal { precision, scale }, "decimal", None)
        }
        _ => (Kind::Text, "decimal", Some(DECIMAL_EXTENSION)),
    }
}

fn blob(
    binary: bool,
    bytes_name: &'static str,
    text_name: &'static str,
) -> (Kind, &'static str, Option<&'static str>) {
    if binary {
        (Kind::Bytes, bytes_name, None)
    } else {
        (Kind::Text, text_name, None)
    }
}

/// The Arrow schema of a result set.
pub(crate) fn schema(columns: &[Column]) -> SchemaRef {
    let fields: Vec<Field> = columns
        .iter()
        .map(|column| {
            let (kind, source, extension) = describe(column);
            let mut metadata = HashMap::from([(SOURCE_TYPE.to_owned(), source.to_owned())]);
            if let Some(extension) = extension {
                metadata.insert(EXTENSION_NAME.to_owned(), extension.to_owned());
            }
            if let Kind::DateTime { fsp: Some(fsp) } | Kind::Time { fsp: Some(fsp) } = kind {
                metadata.insert(FRACTION_DIGITS.to_owned(), fsp.to_string());
            }
            Field::new(column.name_str(), data_type(kind), true).with_metadata(metadata)
        })
        .collect();
    Arc::new(Schema::new(fields))
}

pub(crate) fn data_type(kind: Kind) -> DataType {
    match kind {
        Kind::Int => DataType::Int64,
        Kind::UInt | Kind::Bit => DataType::UInt64,
        Kind::Year => DataType::Int16,
        Kind::Float => DataType::Float32,
        Kind::Double => DataType::Float64,
        Kind::Text => DataType::Utf8,
        Kind::Bytes => DataType::Binary,
        Kind::Date => DataType::Date32,
        Kind::DateTime { .. } => DataType::Timestamp(TimeUnit::Microsecond, None),
        Kind::Time { .. } => DataType::Time64(TimeUnit::Microsecond),
        Kind::Decimal { precision, scale } => DataType::Decimal128(precision, scale),
    }
}

/// The same column as tagged text: `Utf8`, source type kept, marked with
/// [`TEXT_FALLBACK`].
fn tagged_text(field: &Field) -> Field {
    let mut metadata = field.metadata().clone();
    metadata.remove(EXTENSION_NAME);
    metadata.remove(FRACTION_DIGITS);
    metadata.insert(TEXT_FALLBACK.to_owned(), "true".to_owned());
    Field::new(field.name(), DataType::Utf8, true).with_metadata(metadata)
}

#[cfg(test)]
mod tests;
