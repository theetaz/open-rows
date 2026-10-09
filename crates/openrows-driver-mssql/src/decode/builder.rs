// SPDX-License-Identifier: Apache-2.0
//! One Arrow builder per column; a value with no exact Arrow form switches
//! its column to tagged text (docs/04 §5) for the rest of the result.

use std::sync::Arc;

use arrow_array::builder::{
    ArrayBuilder, BinaryBuilder, BooleanBuilder, Date32Builder, Decimal128Builder,
    FixedSizeBinaryBuilder, Float32Builder, Float64Builder, Int16Builder, Int32Builder,
    Int64Builder, StringBuilder, Time64MicrosecondBuilder, Time64NanosecondBuilder,
    TimestampMicrosecondBuilder, TimestampNanosecondBuilder,
};
use arrow_array::cast::AsArray;
use arrow_array::types::TimestampNanosecondType;
use arrow_array::{Array, ArrayRef, RecordBatch};
use arrow_schema::{DataType, Schema, SchemaRef, TimeUnit};
use openrows_driver_api::DriverError;

use super::{CellError, Kind, data_type, kind, number, schema, tagged_text, temporal, text};
use crate::tds::token::{Column, RowValues};

/// One column's builder.
enum Builder {
    Int16(Int16Builder),
    Int32(Int32Builder),
    Int64(Int64Builder),
    Bool(BooleanBuilder),
    Float32(Float32Builder),
    Float64(Float64Builder),
    Decimal(Decimal128Builder),
    Date(Date32Builder),
    TimeMicros(Time64MicrosecondBuilder),
    TimeNanos(Time64NanosecondBuilder),
    StampMicros(TimestampMicrosecondBuilder),
    StampNanos(TimestampNanosecondBuilder),
    Uuid(FixedSizeBinaryBuilder),
    Bytes(BinaryBuilder),
    /// Text of any kind, rendered as the server prints it.
    Text(StringBuilder),
}

/// Rows of the open batch, column by column.
pub(crate) struct BatchBuilder {
    schema: SchemaRef,
    kinds: Vec<Kind>,
    builders: Vec<Builder>,
    rows: usize,
    bytes: usize,
    scratch: String,
}

impl BatchBuilder {
    pub(crate) fn new(columns: &[Column]) -> Self {
        let kinds: Vec<Kind> = columns
            .iter()
            .map(|column| kind(&column.info.sql))
            .collect();
        Self {
            schema: schema(columns),
            builders: kinds.iter().map(|kind| builder(*kind)).collect(),
            kinds,
            rows: 0,
            bytes: 0,
            scratch: String::new(),
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

    /// Append one row.
    ///
    /// # Errors
    /// `Decode` naming the column whose bytes do not fit its type.
    pub(crate) fn append(&mut self, values: &RowValues<'_>) -> Result<(), DriverError> {
        for (column, value) in values.iter().enumerate() {
            self.bytes += value.as_ref().map_or(1, |bytes| bytes.len());
            let (Some(builder), Some(kind)) =
                (self.builders.get_mut(column), self.kinds.get(column))
            else {
                return Err(DriverError::Protocol("row has extra columns".to_owned()));
            };
            match (
                append(builder, *kind, value.as_deref(), &mut self.scratch),
                value,
            ) {
                (Ok(()), _) => {}
                (Err(CellError::Unrepresentable(_)), Some(bytes)) => {
                    self.fall_back(column, bytes)?;
                }
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
    /// as the server prints them, then `value` is appended.
    fn fall_back(&mut self, column: usize, value: &[u8]) -> Result<(), DriverError> {
        let kind = self.kinds[column];
        let mut texts = StringBuilder::new();
        append_as_text(kind, &finish(&mut self.builders[column]), &mut texts);
        let rendered = text::render(kind, value).map_err(|reason| DriverError::Decode {
            column,
            reason: reason.to_string(),
        })?;
        texts.append_value(rendered);
        self.builders[column] = Builder::Text(texts);
        let mut fields: Vec<_> = self.schema.fields().iter().cloned().collect();
        fields[column] = Arc::new(tagged_text(&fields[column]));
        self.schema = Arc::new(Schema::new_with_metadata(
            fields,
            self.schema.metadata().clone(),
        ));
        Ok(())
    }

    /// Finish the open batch and start a new one.
    ///
    /// # Errors
    /// Arrow rejects the batch.
    pub(crate) fn finish(&mut self) -> Result<RecordBatch, DriverError> {
        let arrays: Vec<ArrayRef> = self.builders.iter_mut().map(finish).collect();
        let fields = self.schema.fields();
        for ((slot, kind), field) in self.builders.iter_mut().zip(&self.kinds).zip(fields) {
            // A column that fell back to text stays text.
            *slot = if field.data_type() == &DataType::Utf8 {
                Builder::Text(StringBuilder::new())
            } else {
                builder(*kind)
            };
        }
        self.rows = 0;
        self.bytes = 0;
        Ok(RecordBatch::try_new(Arc::clone(&self.schema), arrays)?)
    }
}

fn builder(kind: Kind) -> Builder {
    match data_type(kind) {
        DataType::Int16 => Builder::Int16(Int16Builder::new()),
        DataType::Int32 => Builder::Int32(Int32Builder::new()),
        DataType::Int64 => Builder::Int64(Int64Builder::new()),
        DataType::Boolean => Builder::Bool(BooleanBuilder::new()),
        DataType::Float32 => Builder::Float32(Float32Builder::new()),
        DataType::Float64 => Builder::Float64(Float64Builder::new()),
        data @ DataType::Decimal128(..) => {
            Builder::Decimal(Decimal128Builder::new().with_data_type(data))
        }
        DataType::Date32 => Builder::Date(Date32Builder::new()),
        DataType::Time64(TimeUnit::Nanosecond) => {
            Builder::TimeNanos(Time64NanosecondBuilder::new())
        }
        DataType::Time64(_) => Builder::TimeMicros(Time64MicrosecondBuilder::new()),
        DataType::Timestamp(TimeUnit::Nanosecond, _) => {
            Builder::StampNanos(TimestampNanosecondBuilder::new())
        }
        DataType::Timestamp(..) => Builder::StampMicros(TimestampMicrosecondBuilder::new()),
        DataType::FixedSizeBinary(_) => Builder::Uuid(FixedSizeBinaryBuilder::new(16)),
        DataType::Binary => Builder::Bytes(BinaryBuilder::new()),
        _ => Builder::Text(StringBuilder::new()),
    }
}

fn finish(builder: &mut Builder) -> ArrayRef {
    match builder {
        Builder::Int16(b) => Arc::new(b.finish()),
        Builder::Int32(b) => Arc::new(b.finish()),
        Builder::Int64(b) => Arc::new(b.finish()),
        Builder::Bool(b) => Arc::new(b.finish()),
        Builder::Float32(b) => Arc::new(b.finish()),
        Builder::Float64(b) => Arc::new(b.finish()),
        Builder::Decimal(b) => Arc::new(b.finish()),
        Builder::Date(b) => Arc::new(b.finish()),
        Builder::TimeMicros(b) => Arc::new(b.finish()),
        Builder::TimeNanos(b) => Arc::new(b.finish()),
        Builder::StampMicros(b) => Arc::new(b.finish()),
        Builder::StampNanos(b) => Arc::new(b.finish()),
        Builder::Uuid(b) => Arc::new(b.finish()),
        Builder::Bytes(b) => ArrayBuilder::finish(b),
        Builder::Text(b) => Arc::new(b.finish()),
    }
}

/// Append one value; `None` is NULL.
fn append(
    builder: &mut Builder,
    kind: Kind,
    value: Option<&[u8]>,
    scratch: &mut String,
) -> Result<(), CellError> {
    let Some(bytes) = value else {
        append_null(builder);
        return Ok(());
    };
    match builder {
        Builder::Int16(b) => b.append_value(match kind {
            Kind::TinyInt => number::tinyint(bytes)?,
            _ => number::smallint(bytes)?,
        }),
        Builder::Int32(b) => b.append_value(number::int(bytes)?),
        Builder::Int64(b) => b.append_value(number::bigint(bytes)?),
        Builder::Bool(b) => b.append_value(number::bit(bytes)?),
        Builder::Float32(b) => b.append_value(number::real(bytes)?),
        Builder::Float64(b) => b.append_value(number::float(bytes)?),
        Builder::Decimal(b) => b.append_value(match kind {
            Kind::Money | Kind::SmallMoney => number::money(bytes)?,
            _ => number::decimal(bytes)?,
        }),
        Builder::Uuid(b) => b
            .append_value(number::guid(bytes)?)
            .map_err(|error| CellError::Malformed(error.to_string()))?,
        Builder::Bytes(b) => b.append_value(bytes),
        Builder::Text(b) => {
            scratch.clear();
            text::render_into(kind, bytes, scratch)?;
            b.append_value(scratch.as_str());
        }
        temporal_builder => append_temporal(temporal_builder, kind, bytes)?,
    }
    Ok(())
}

fn append_temporal(builder: &mut Builder, kind: Kind, bytes: &[u8]) -> Result<(), CellError> {
    match (builder, kind) {
        (Builder::Date(b), _) => {
            let days = temporal::date_days(bytes)?;
            b.append_value(i32::try_from(days).map_err(|_| CellError::Unrepresentable("date"))?);
        }
        (Builder::TimeMicros(b), Kind::Time { scale }) => {
            b.append_value(temporal::time_nanos(bytes, scale)? / 1_000);
        }
        (Builder::TimeNanos(b), Kind::Time { scale }) => {
            b.append_value(temporal::time_nanos(bytes, scale)?);
        }
        (Builder::StampNanos(b), Kind::DateTime2 { scale }) => {
            b.append_value(temporal::datetime2_nanos(bytes, scale)?);
        }
        (Builder::StampMicros(b), Kind::DateTime2 { scale }) => {
            b.append_value(temporal::datetime2_micros(bytes, scale)?);
        }
        (Builder::StampMicros(b), Kind::DateTime) => {
            b.append_value(temporal::datetime_micros(bytes)?);
        }
        (Builder::StampMicros(b), Kind::SmallDateTime) => {
            b.append_value(temporal::smalldatetime_micros(bytes)?);
        }
        _ => {
            return Err(CellError::Malformed(
                "value does not fit its column".to_owned(),
            ));
        }
    }
    Ok(())
}

fn append_null(builder: &mut Builder) {
    match builder {
        Builder::Int16(b) => b.append_null(),
        Builder::Int32(b) => b.append_null(),
        Builder::Int64(b) => b.append_null(),
        Builder::Bool(b) => b.append_null(),
        Builder::Float32(b) => b.append_null(),
        Builder::Float64(b) => b.append_null(),
        Builder::Decimal(b) => b.append_null(),
        Builder::Date(b) => b.append_null(),
        Builder::TimeMicros(b) => b.append_null(),
        Builder::TimeNanos(b) => b.append_null(),
        Builder::StampMicros(b) => b.append_null(),
        Builder::StampNanos(b) => b.append_null(),
        Builder::Uuid(b) => b.append_null(),
        Builder::Bytes(b) => b.append_null(),
        Builder::Text(b) => b.append_null(),
    }
}

/// The typed values of a column that falls back, printed as the server
/// prints them. Only `datetime2(7)` (nanoseconds) falls back.
fn append_as_text(kind: Kind, values: &dyn Array, out: &mut StringBuilder) {
    let (Kind::DateTime2 { scale }, Some(stamps)) =
        (kind, values.as_primitive_opt::<TimestampNanosecondType>())
    else {
        return;
    };
    let mut text = String::new();
    for value in stamps {
        match value {
            Some(nanos) => {
                text.clear();
                let fraction = (nanos.rem_euclid(1_000), scale);
                temporal::write_timestamp(nanos.div_euclid(1_000), fraction, &mut text);
                out.append_value(&text);
            }
            None => out.append_null(),
        }
    }
}

#[cfg(test)]
#[path = "builder_tests.rs"]
mod tests;
