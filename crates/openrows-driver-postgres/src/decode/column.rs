// SPDX-License-Identifier: Apache-2.0
//! One typed Arrow builder per column; enum dispatch keeps the per-cell path static.
use super::array::ListValues;
use super::numeric::{self, Numeric};
use super::scalar::{self, CellError, array};
use super::temporal::{self, Interval};
use super::{fallback, text};
use crate::types::ColumnKind;
use arrow_array::ArrayRef;
use arrow_array::builder::{
    BinaryBuilder, BooleanBuilder, Date32Builder, Decimal128Builder, FixedSizeBinaryBuilder,
    Float32Builder, Float64Builder, Int16Builder, Int32Builder, Int64Builder,
    IntervalMonthDayNanoBuilder, StringBuilder, Time64MicrosecondBuilder,
    TimestampMicrosecondBuilder,
};
use std::sync::Arc;

/// Initial byte capacity per variable-width cell before any batch has been seen.
const DEFAULT_CELL_BYTES: usize = 16;

pub(super) enum Builder {
    Int16(Int16Builder),
    Int32(Int32Builder),
    Int64(Int64Builder),
    Float32(Float32Builder),
    Float64(Float64Builder),
    Bool(BooleanBuilder),
    Text(StringBuilder),
    Jsonb(StringBuilder),
    Numeric(StringBuilder),
    Binary(BinaryBuilder),
    Timestamp(TimestampMicrosecondBuilder),
    Date(Date32Builder),
    Uuid(FixedSizeBinaryBuilder),
    Decimal(Decimal128Builder, i8),
    Time(Time64MicrosecondBuilder),
    Interval(IntervalMonthDayNanoBuilder),
    /// Binary values stored as their text form (`timetz`, `inet`, `macaddr`).
    Formatted(StringBuilder, ColumnKind),
    List(Box<ListValues>),
    /// Tagged-text fallback: every value of the column as `PostgreSQL` text.
    Tagged(StringBuilder, ColumnKind),
}

/// Pre-sized builder for one column, re-created after each batch.
pub(crate) struct ColumnDecoder {
    kind: ColumnKind,
    tagged: bool,
    builder: Builder,
}

impl ColumnDecoder {
    pub(crate) fn new(kind: ColumnKind, rows: usize) -> Self {
        Self {
            kind,
            tagged: false,
            builder: Builder::new(kind, rows, rows * DEFAULT_CELL_BYTES),
        }
    }

    /// Switch to tagged text for the rest of the result after `value` failed with
    /// `reason`: converts the open batch's values, then appends `value` as text.
    /// Returns `reason` unchanged when this column cannot fall back.
    pub(crate) fn fall_back(&mut self, value: &[u8], reason: CellError) -> Result<(), CellError> {
        if self.tagged || !fallback::supports(self.kind) {
            return Err(reason);
        }
        let mut text = StringBuilder::with_capacity(DEFAULT_CELL_BYTES, DEFAULT_CELL_BYTES * 32);
        fallback::append_array(self.kind, self.builder.finish().as_ref(), &mut text)?;
        fallback::append_wire(self.kind, value, &mut text)?;
        (self.builder, self.tagged) = (Builder::Tagged(text, self.kind), true);
        Ok(())
    }

    /// Append one wire value (`None` = SQL NULL).
    #[inline]
    pub(crate) fn append(&mut self, value: Option<&[u8]>) -> Result<(), CellError> {
        if let Some(bytes) = value {
            self.builder.append_value(bytes)
        } else {
            self.builder.append_null();
            Ok(())
        }
    }

    /// Finish the batch column and pre-size the next one from this batch's bytes.
    pub(crate) fn finish(&mut self, next_rows: usize) -> ArrayRef {
        let bytes = self
            .builder
            .data_bytes()
            .max(next_rows * DEFAULT_CELL_BYTES);
        let next = if self.tagged {
            Builder::Tagged(StringBuilder::with_capacity(next_rows, bytes), self.kind)
        } else {
            Builder::new(self.kind, next_rows, bytes)
        };
        std::mem::replace(&mut self.builder, next).finish()
    }
}

impl Builder {
    pub(super) fn new(kind: ColumnKind, rows: usize, bytes: usize) -> Self {
        match kind {
            ColumnKind::Int2 => Self::Int16(Int16Builder::with_capacity(rows)),
            ColumnKind::Int4 => Self::Int32(Int32Builder::with_capacity(rows)),
            ColumnKind::Int8 => Self::Int64(Int64Builder::with_capacity(rows)),
            ColumnKind::Float4 => Self::Float32(Float32Builder::with_capacity(rows)),
            ColumnKind::Float8 => Self::Float64(Float64Builder::with_capacity(rows)),
            ColumnKind::Bool => Self::Bool(BooleanBuilder::with_capacity(rows)),
            ColumnKind::Text | ColumnKind::Json | ColumnKind::Range | ColumnKind::Unknown => {
                Self::Text(StringBuilder::with_capacity(rows, bytes))
            }
            ColumnKind::TimeTz | ColumnKind::Inet | ColumnKind::MacAddr => {
                Self::Formatted(StringBuilder::with_capacity(rows, bytes), kind)
            }
            ColumnKind::Time => Self::Time(Time64MicrosecondBuilder::with_capacity(rows)),
            ColumnKind::Interval => {
                Self::Interval(IntervalMonthDayNanoBuilder::with_capacity(rows))
            }
            ColumnKind::Array { .. } => match ListValues::new(kind, rows, bytes) {
                Some(list) => Self::List(Box::new(list)),
                None => Self::Text(StringBuilder::with_capacity(rows, bytes)),
            },
            ColumnKind::Jsonb => Self::Jsonb(StringBuilder::with_capacity(rows, bytes)),
            ColumnKind::NumericText => Self::Numeric(StringBuilder::with_capacity(rows, bytes)),
            ColumnKind::Bytea => Self::Binary(BinaryBuilder::with_capacity(rows, bytes)),
            ColumnKind::Timestamp => {
                Self::Timestamp(TimestampMicrosecondBuilder::with_capacity(rows))
            }
            ColumnKind::TimestampTz => Self::Timestamp(
                TimestampMicrosecondBuilder::with_capacity(rows).with_timezone("UTC"),
            ),
            ColumnKind::Date => Self::Date(Date32Builder::with_capacity(rows)),
            ColumnKind::Uuid => Self::Uuid(FixedSizeBinaryBuilder::with_capacity(rows, 16)),
            ColumnKind::Decimal { scale, .. } => Self::Decimal(
                Decimal128Builder::with_capacity(rows).with_data_type(kind.data_type()),
                scale,
            ),
        }
    }

    #[inline]
    pub(super) fn append_value(&mut self, bytes: &[u8]) -> Result<(), CellError> {
        match self {
            Self::Int16(b) => b.append_value(i16::from_be_bytes(array(bytes)?)),
            Self::Int32(b) => b.append_value(i32::from_be_bytes(array(bytes)?)),
            Self::Int64(b) => b.append_value(i64::from_be_bytes(array(bytes)?)),
            Self::Float32(b) => b.append_value(f32::from_be_bytes(array(bytes)?)),
            Self::Float64(b) => b.append_value(f64::from_be_bytes(array(bytes)?)),
            Self::Bool(b) => b.append_value(array::<1>(bytes)?[0] != 0),
            Self::Text(b) => b.append_value(scalar::utf8(bytes)?),
            Self::Jsonb(b) => b.append_value(scalar::jsonb(bytes)?),
            Self::Numeric(b) => append_numeric_text(b, bytes)?,
            Self::Binary(b) => b.append_value(bytes),
            Self::Timestamp(b) => b.append_value(scalar::timestamp_micros(bytes)?),
            Self::Date(b) => b.append_value(scalar::date_days(bytes)?),
            Self::Uuid(b) => b
                .append_value(array::<16>(bytes)?)
                .map_err(|_| CellError::Truncated)?,
            Self::Decimal(b, scale) => b.append_value(decimal(bytes, *scale)?),
            Self::Time(b) => b.append_value(temporal::time_micros(bytes)?),
            Self::Interval(b) => b.append_value(Interval::parse(bytes)?.month_day_nano()?),
            Self::Formatted(b, kind) => {
                text::write_wire(*kind, bytes, b)?;
                b.append_value("");
            }
            Self::List(list) => list.append_value(bytes)?,
            Self::Tagged(b, kind) => fallback::append_wire(*kind, bytes, b)?,
        }
        Ok(())
    }

    pub(super) fn append_null(&mut self) {
        match self {
            Self::Int16(b) => b.append_null(),
            Self::Int32(b) => b.append_null(),
            Self::Int64(b) => b.append_null(),
            Self::Float32(b) => b.append_null(),
            Self::Float64(b) => b.append_null(),
            Self::Bool(b) => b.append_null(),
            Self::Text(b)
            | Self::Jsonb(b)
            | Self::Numeric(b)
            | Self::Formatted(b, _)
            | Self::Tagged(b, _) => {
                b.append_null();
            }
            Self::Time(b) => b.append_null(),
            Self::Interval(b) => b.append_null(),
            Self::List(list) => list.append_null(),
            Self::Binary(b) => b.append_null(),
            Self::Timestamp(b) => b.append_null(),
            Self::Date(b) => b.append_null(),
            Self::Uuid(b) => b.append_null(),
            Self::Decimal(b, _) => b.append_null(),
        }
    }

    pub(super) fn data_bytes(&self) -> usize {
        match self {
            Self::Text(b)
            | Self::Jsonb(b)
            | Self::Numeric(b)
            | Self::Formatted(b, _)
            | Self::Tagged(b, _) => b.values_slice().len(),
            Self::List(list) => list.data_bytes(),
            Self::Binary(b) => b.values_slice().len(),
            _ => 0,
        }
    }

    /// Finish the values so far; the builder is left empty.
    pub(super) fn finish(&mut self) -> ArrayRef {
        match self {
            Self::Int16(b) => Arc::new(b.finish()),
            Self::Int32(b) => Arc::new(b.finish()),
            Self::Int64(b) => Arc::new(b.finish()),
            Self::Float32(b) => Arc::new(b.finish()),
            Self::Float64(b) => Arc::new(b.finish()),
            Self::Bool(b) => Arc::new(b.finish()),
            Self::Text(b)
            | Self::Jsonb(b)
            | Self::Numeric(b)
            | Self::Formatted(b, _)
            | Self::Tagged(b, _) => Arc::new(b.finish()),
            Self::Time(b) => Arc::new(b.finish()),
            Self::Interval(b) => Arc::new(b.finish()),
            Self::List(list) => list.finish(),
            Self::Binary(b) => Arc::new(b.finish()),
            Self::Timestamp(b) => Arc::new(b.finish()),
            Self::Date(b) => Arc::new(b.finish()),
            Self::Uuid(b) => Arc::new(b.finish()),
            Self::Decimal(b, _) => Arc::new(b.finish()),
        }
    }
}

fn decimal(bytes: &[u8], scale: i8) -> Result<i128, CellError> {
    match numeric::parse(bytes)? {
        Numeric::Finite(finite) => numeric::to_decimal(&finite, scale),
        _ => Err(CellError::Unrepresentable("numeric NaN/Infinity")),
    }
}

fn append_numeric_text(builder: &mut StringBuilder, bytes: &[u8]) -> Result<(), CellError> {
    let value = numeric::parse(bytes)?;
    numeric::write_text(&value, builder).map_err(|_| CellError::Numeric)?;
    builder.append_value("");
    Ok(())
}
