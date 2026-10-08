// SPDX-License-Identifier: Apache-2.0
//! Result sets into Arrow batches for the result pipeline (docs/05 §2).
//!
//! | `MySQL` type | Arrow | Field metadata |
//! | --- | --- | --- |
//! | `TINYINT` … `INT`, `YEAR`, signed `BIGINT` | `Int64` | |
//! | unsigned `BIGINT` | `UInt64` | |
//! | `FLOAT` / `DOUBLE` | `Float32` / `Float64` | |
//! | `DECIMAL` | `Utf8`, exact text | `openrows.decimal` |
//! | `BIT(n)` | `UInt64` | |
//! | `JSON` | `Utf8` | `openrows.json` |
//! | `DATE`, `TIME`, `DATETIME`, `TIMESTAMP` | `Utf8`, server text | |
//! | text types, `ENUM`, `SET` | `Utf8` | |
//! | binary strings, `BLOB`, `GEOMETRY` | `Binary` | |
//!
//! Temporal values keep the server's text: zero dates (`0000-00-00`) and
//! `TIME` beyond 24 hours have no exact Arrow form. Every field carries
//! `openrows.source_type`.
use std::collections::HashMap;
use std::sync::Arc;

use arrow_array::builder::{
    ArrayBuilder, BinaryBuilder, Float32Builder, Float64Builder, Int64Builder, StringBuilder,
    UInt64Builder,
};
use arrow_array::{ArrayRef, RecordBatch};
use arrow_schema::{DataType, Field, Schema, SchemaRef};
use mysql_common::constants::{ColumnFlags, ColumnType};
use mysql_common::packets::Column;
use openrows_driver_api::DriverError;
use openrows_driver_api::metadata::{
    DECIMAL_EXTENSION, EXTENSION_NAME, JSON_EXTENSION, SOURCE_TYPE,
};

use crate::values::unsigned;

/// The `binary` character set: bytes, not text.
const BINARY_CHARSET: u16 = 63;

/// How one column's text values become Arrow values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Kind {
    Int,
    UInt,
    Float,
    Double,
    Bit,
    Text,
    Bytes,
}

/// Arrow type, kind, source type name and extension of `column`.
pub(crate) fn describe(column: &Column) -> (Kind, &'static str, Option<&'static str>) {
    use ColumnType as T;
    let binary = column.character_set() == BINARY_CHARSET;
    let flags = column.flags();
    match column.column_type() {
        T::MYSQL_TYPE_TINY => (Kind::Int, "tinyint", None),
        T::MYSQL_TYPE_SHORT => (Kind::Int, "smallint", None),
        T::MYSQL_TYPE_INT24 => (Kind::Int, "mediumint", None),
        T::MYSQL_TYPE_LONG => (Kind::Int, "int", None),
        T::MYSQL_TYPE_YEAR => (Kind::Int, "year", None),
        T::MYSQL_TYPE_LONGLONG if unsigned(column) => (Kind::UInt, "bigint unsigned", None),
        T::MYSQL_TYPE_LONGLONG => (Kind::Int, "bigint", None),
        T::MYSQL_TYPE_FLOAT => (Kind::Float, "float", None),
        T::MYSQL_TYPE_DOUBLE => (Kind::Double, "double", None),
        T::MYSQL_TYPE_DECIMAL | T::MYSQL_TYPE_NEWDECIMAL => {
            (Kind::Text, "decimal", Some(DECIMAL_EXTENSION))
        }
        T::MYSQL_TYPE_BIT => (Kind::Bit, "bit", None),
        T::MYSQL_TYPE_JSON => (Kind::Text, "json", Some(JSON_EXTENSION)),
        T::MYSQL_TYPE_DATE | T::MYSQL_TYPE_NEWDATE => (Kind::Text, "date", None),
        T::MYSQL_TYPE_TIME | T::MYSQL_TYPE_TIME2 => (Kind::Text, "time", None),
        T::MYSQL_TYPE_DATETIME | T::MYSQL_TYPE_DATETIME2 => (Kind::Text, "datetime", None),
        T::MYSQL_TYPE_TIMESTAMP | T::MYSQL_TYPE_TIMESTAMP2 => (Kind::Text, "timestamp", None),
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
            Field::new(column.name_str(), data_type(kind), true).with_metadata(metadata)
        })
        .collect();
    Arc::new(Schema::new(fields))
}

fn data_type(kind: Kind) -> DataType {
    match kind {
        Kind::Int => DataType::Int64,
        Kind::UInt | Kind::Bit => DataType::UInt64,
        Kind::Float => DataType::Float32,
        Kind::Double => DataType::Float64,
        Kind::Text => DataType::Utf8,
        Kind::Bytes => DataType::Binary,
    }
}

/// One column's builder.
enum Builder {
    Int(Int64Builder),
    UInt(UInt64Builder),
    Bit(UInt64Builder),
    Float(Float32Builder),
    Double(Float64Builder),
    Text(StringBuilder),
    Bytes(BinaryBuilder),
}

/// Rows of the open batch, column by column.
pub(crate) struct BatchBuilder {
    schema: SchemaRef,
    kinds: Vec<Kind>,
    builders: Vec<Builder>,
    rows: usize,
    bytes: usize,
}

impl BatchBuilder {
    pub(crate) fn new(columns: &[Column]) -> Self {
        let kinds: Vec<Kind> = columns.iter().map(|column| describe(column).0).collect();
        Self {
            schema: schema(columns),
            builders: kinds.iter().map(|kind| builder(*kind)).collect(),
            kinds,
            rows: 0,
            bytes: 0,
        }
    }

    pub(crate) fn schema(&self) -> SchemaRef {
        Arc::clone(&self.schema)
    }

    pub(crate) fn rows(&self) -> usize {
        self.rows
    }

    pub(crate) fn bytes(&self) -> usize {
        self.bytes
    }

    /// Append one row of text values.
    ///
    /// # Errors
    /// `Decode` naming the column whose text is not a valid number.
    pub(crate) fn append(&mut self, values: &[Option<Vec<u8>>]) -> Result<(), DriverError> {
        for (column, (builder, value)) in self.builders.iter_mut().zip(values).enumerate() {
            self.bytes += value.as_ref().map_or(1, Vec::len);
            append(builder, value.as_deref())
                .map_err(|reason| DriverError::Decode { column, reason })?;
        }
        self.rows += 1;
        Ok(())
    }

    /// Finish the open batch and start a new one.
    ///
    /// # Errors
    /// Arrow rejects the batch.
    pub(crate) fn finish(&mut self) -> Result<RecordBatch, DriverError> {
        let arrays: Vec<ArrayRef> = self.builders.iter_mut().map(finish).collect();
        self.builders = self.kinds.iter().map(|kind| builder(*kind)).collect();
        self.rows = 0;
        self.bytes = 0;
        Ok(RecordBatch::try_new(Arc::clone(&self.schema), arrays)?)
    }
}

fn builder(kind: Kind) -> Builder {
    match kind {
        Kind::Int => Builder::Int(Int64Builder::new()),
        Kind::UInt => Builder::UInt(UInt64Builder::new()),
        Kind::Bit => Builder::Bit(UInt64Builder::new()),
        Kind::Float => Builder::Float(Float32Builder::new()),
        Kind::Double => Builder::Double(Float64Builder::new()),
        Kind::Text => Builder::Text(StringBuilder::new()),
        Kind::Bytes => Builder::Bytes(BinaryBuilder::new()),
    }
}

fn finish(builder: &mut Builder) -> ArrayRef {
    match builder {
        Builder::Int(b) => Arc::new(b.finish()),
        Builder::UInt(b) | Builder::Bit(b) => Arc::new(b.finish()),
        Builder::Float(b) => Arc::new(b.finish()),
        Builder::Double(b) => Arc::new(b.finish()),
        Builder::Text(b) => Arc::new(b.finish()),
        Builder::Bytes(b) => ArrayBuilder::finish(b),
    }
}

/// Parse the text form of a number.
fn number<T: std::str::FromStr>(bytes: &[u8]) -> Result<T, String> {
    std::str::from_utf8(bytes)
        .ok()
        .and_then(|text| text.trim().parse().ok())
        .ok_or_else(|| format!("{:?} is not a number", String::from_utf8_lossy(bytes)))
}

fn append(builder: &mut Builder, value: Option<&[u8]>) -> Result<(), String> {
    let Some(bytes) = value else {
        match builder {
            Builder::Int(b) => b.append_null(),
            Builder::UInt(b) | Builder::Bit(b) => b.append_null(),
            Builder::Float(b) => b.append_null(),
            Builder::Double(b) => b.append_null(),
            Builder::Text(b) => b.append_null(),
            Builder::Bytes(b) => b.append_null(),
        }
        return Ok(());
    };
    match builder {
        Builder::Int(b) => b.append_value(number(bytes)?),
        Builder::UInt(b) => b.append_value(number(bytes)?),
        Builder::Bit(b) => b.append_value(bits(bytes)?),
        Builder::Float(b) => b.append_value(number(bytes)?),
        Builder::Double(b) => b.append_value(number(bytes)?),
        Builder::Text(b) => b.append_value(String::from_utf8_lossy(bytes)),
        Builder::Bytes(b) => b.append_value(bytes),
    }
    Ok(())
}

/// `BIT(n)` arrives as big-endian bytes in both protocols.
fn bits(bytes: &[u8]) -> Result<u64, String> {
    if bytes.len() > 8 {
        return Err("BIT value wider than 64 bits".to_owned());
    }
    Ok(bytes
        .iter()
        .fold(0_u64, |value, byte| value << 8 | u64::from(*byte)))
}

#[cfg(test)]
mod tests;
