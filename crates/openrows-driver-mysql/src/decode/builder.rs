// SPDX-License-Identifier: Apache-2.0
//! One Arrow builder per column; a value with no exact Arrow form switches
//! its column to tagged text (docs/04 §5) for the rest of the result.
use std::sync::Arc;

use arrow_array::builder::{
    ArrayBuilder, BinaryBuilder, Date32Builder, Decimal128Builder, Float32Builder, Float64Builder,
    Int16Builder, Int64Builder, StringBuilder, Time64MicrosecondBuilder,
    TimestampMicrosecondBuilder, UInt64Builder,
};
use arrow_array::{ArrayRef, RecordBatch};
use arrow_schema::{Schema, SchemaRef};
use mysql_common::packets::Column;
use openrows_driver_api::DriverError;

use super::fallback;
use super::{CellError, Kind, data_type, decimal, describe, schema, tagged_text, temporal};

/// One column's builder.
enum Builder {
    Int(Int64Builder),
    UInt(UInt64Builder),
    Year(Int16Builder),
    Bit(UInt64Builder),
    Float(Float32Builder),
    Double(Float64Builder),
    Text(StringBuilder),
    Bytes(BinaryBuilder),
    Date(Date32Builder),
    DateTime(TimestampMicrosecondBuilder),
    Time(Time64MicrosecondBuilder),
    Decimal(Decimal128Builder, u8, i8),
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

    /// The current schema; it changes when a column falls back to text.
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
        for (column, value) in values.iter().enumerate() {
            self.bytes += value.as_ref().map_or(1, Vec::len);
            let Some(builder) = self.builders.get_mut(column) else {
                return Err(DriverError::Protocol("row has extra columns".to_owned()));
            };
            match (append(builder, value.as_deref()), value) {
                (Ok(()), _) => {}
                (Err(CellError::Unrepresentable(_)), Some(bytes)) => self.fall_back(column, bytes),
                (Err(reason), _) => {
                    return Err(DriverError::Decode {
                        column,
                        reason: reason.to_string(),
                    });
                }
            }
        }
        self.rows += 1;
        Ok(())
    }

    /// Switch `column` to tagged text: the open batch's values are printed
    /// as the server prints them, then `value` is appended as sent.
    fn fall_back(&mut self, column: usize, value: &[u8]) {
        let mut text = StringBuilder::new();
        fallback::append_array(
            self.kinds[column],
            &finish(&mut self.builders[column]),
            &mut text,
        );
        text.append_value(String::from_utf8_lossy(value));
        self.builders[column] = Builder::Text(text);
        self.kinds[column] = Kind::Text;
        let mut fields: Vec<_> = self.schema.fields().iter().cloned().collect();
        fields[column] = Arc::new(tagged_text(&fields[column]));
        self.schema = Arc::new(Schema::new_with_metadata(
            fields,
            self.schema.metadata().clone(),
        ));
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
        Kind::Year => Builder::Year(Int16Builder::new()),
        Kind::Bit => Builder::Bit(UInt64Builder::new()),
        Kind::Float => Builder::Float(Float32Builder::new()),
        Kind::Double => Builder::Double(Float64Builder::new()),
        Kind::Text => Builder::Text(StringBuilder::new()),
        Kind::Bytes => Builder::Bytes(BinaryBuilder::new()),
        Kind::Date => Builder::Date(Date32Builder::new()),
        Kind::DateTime { .. } => Builder::DateTime(TimestampMicrosecondBuilder::new()),
        Kind::Time { .. } => Builder::Time(Time64MicrosecondBuilder::new()),
        Kind::Decimal { precision, scale } => Builder::Decimal(
            Decimal128Builder::new().with_data_type(data_type(kind)),
            precision,
            scale,
        ),
    }
}

fn finish(builder: &mut Builder) -> ArrayRef {
    match builder {
        Builder::Int(b) => Arc::new(b.finish()),
        Builder::UInt(b) | Builder::Bit(b) => Arc::new(b.finish()),
        Builder::Year(b) => Arc::new(b.finish()),
        Builder::Float(b) => Arc::new(b.finish()),
        Builder::Double(b) => Arc::new(b.finish()),
        Builder::Text(b) => Arc::new(b.finish()),
        Builder::Bytes(b) => ArrayBuilder::finish(b),
        Builder::Date(b) => Arc::new(b.finish()),
        Builder::DateTime(b) => Arc::new(b.finish()),
        Builder::Time(b) => Arc::new(b.finish()),
        Builder::Decimal(b, ..) => Arc::new(b.finish()),
    }
}

/// Parse the text form of a number.
fn number<T: std::str::FromStr>(bytes: &[u8]) -> Result<T, CellError> {
    std::str::from_utf8(bytes)
        .ok()
        .and_then(|text| text.trim().parse().ok())
        .ok_or_else(|| {
            CellError::Malformed(format!(
                "{:?} is not a number",
                String::from_utf8_lossy(bytes)
            ))
        })
}

fn append(builder: &mut Builder, value: Option<&[u8]>) -> Result<(), CellError> {
    let Some(bytes) = value else {
        append_null(builder);
        return Ok(());
    };
    match builder {
        Builder::Int(b) => b.append_value(number(bytes)?),
        Builder::UInt(b) => b.append_value(number(bytes)?),
        Builder::Year(b) => b.append_value(number(bytes)?),
        Builder::Bit(b) => b.append_value(bits(bytes)?),
        Builder::Float(b) => b.append_value(number(bytes)?),
        Builder::Double(b) => b.append_value(number(bytes)?),
        Builder::Text(b) => b.append_value(String::from_utf8_lossy(bytes)),
        Builder::Bytes(b) => b.append_value(bytes),
        Builder::Date(b) => b.append_value(temporal::date_days(bytes)?),
        Builder::DateTime(b) => b.append_value(temporal::datetime_micros(bytes)?),
        Builder::Time(b) => b.append_value(temporal::time_micros(bytes)?),
        Builder::Decimal(b, precision, scale) => {
            b.append_value(decimal::parse(bytes, *precision, *scale)?);
        }
    }
    Ok(())
}

fn append_null(builder: &mut Builder) {
    match builder {
        Builder::Int(b) => b.append_null(),
        Builder::UInt(b) | Builder::Bit(b) => b.append_null(),
        Builder::Year(b) => b.append_null(),
        Builder::Float(b) => b.append_null(),
        Builder::Double(b) => b.append_null(),
        Builder::Text(b) => b.append_null(),
        Builder::Bytes(b) => b.append_null(),
        Builder::Date(b) => b.append_null(),
        Builder::DateTime(b) => b.append_null(),
        Builder::Time(b) => b.append_null(),
        Builder::Decimal(b, ..) => b.append_null(),
    }
}

/// `BIT(n)` arrives as big-endian bytes in both protocols.
fn bits(bytes: &[u8]) -> Result<u64, CellError> {
    if bytes.len() > 8 {
        return Err(CellError::Malformed(
            "BIT value wider than 64 bits".to_owned(),
        ));
    }
    Ok(bytes
        .iter()
        .fold(0_u64, |value, byte| value << 8 | u64::from(*byte)))
}
