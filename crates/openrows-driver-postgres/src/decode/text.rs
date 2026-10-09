// SPDX-License-Identifier: Apache-2.0
//! `PostgreSQL` text output of one scalar value, written either from its
//! binary wire form or from the Arrow value it was decoded into. Array text
//! and tagged-text fallback are built from these.
use super::fallback::{write_date, write_decimal, write_timestamp};
use super::scalar::{self, CellError, PG_EPOCH_DAYS, PG_EPOCH_MICROS, array};
use super::temporal::{self, Interval};
use super::{network, numeric};
use crate::types::ColumnKind;
use arrow_array::cast::AsArray;
use arrow_array::types::{
    Date32Type, Decimal128Type, Float32Type, Float64Type, Int16Type, Int32Type, Int64Type,
    IntervalMonthDayNanoType, Time64MicrosecondType, TimestampMicrosecondType,
};
use arrow_array::{Array, ArrowPrimitiveType};
use std::fmt::{self, Display, Write};

/// Write the text of one binary wire value of scalar `kind`.
pub(crate) fn write_wire(
    kind: ColumnKind,
    bytes: &[u8],
    out: &mut impl Write,
) -> Result<(), CellError> {
    let format = |result: fmt::Result| result.map_err(|_| CellError::Format);
    match kind {
        ColumnKind::Int2 => format(write!(out, "{}", i16::from_be_bytes(array(bytes)?))),
        ColumnKind::Int4 => format(write!(out, "{}", i32::from_be_bytes(array(bytes)?))),
        ColumnKind::Int8 => format(write!(out, "{}", i64::from_be_bytes(array(bytes)?))),
        ColumnKind::Float4 => format(write_float(f32::from_be_bytes(array(bytes)?), 6, out)),
        ColumnKind::Float8 => format(write_float(f64::from_be_bytes(array(bytes)?), 15, out)),
        ColumnKind::Bool => format(out.write_str(bool_text(array::<1>(bytes)?[0] != 0))),
        ColumnKind::Bytea => format(write_hex(bytes, out)),
        ColumnKind::Uuid => format(write_uuid(&array::<16>(bytes)?, out)),
        ColumnKind::Jsonb => format(out.write_str(scalar::jsonb(bytes)?)),
        ColumnKind::Timestamp | ColumnKind::TimestampTz => format(write_timestamp(
            i64::from_be_bytes(array(bytes)?),
            kind == ColumnKind::TimestampTz,
            out,
        )),
        ColumnKind::Date => format(write_date(i32::from_be_bytes(array(bytes)?), out)),
        ColumnKind::Time => format(temporal::write_clock(
            i64::from_be_bytes(array(bytes)?),
            out,
        )),
        ColumnKind::TimeTz => temporal::write_time_tz(bytes, out),
        ColumnKind::Interval => format(Interval::parse(bytes)?.write(out)),
        ColumnKind::Decimal { .. } | ColumnKind::NumericText => {
            format(numeric::write_text(&numeric::parse(bytes)?, out))
        }
        ColumnKind::Inet => network::write_inet(bytes, out),
        ColumnKind::MacAddr => network::write_macaddr(bytes, out),
        ColumnKind::Text
        | ColumnKind::Json
        | ColumnKind::Range
        | ColumnKind::Unknown
        | ColumnKind::Array { .. } => format(out.write_str(scalar::utf8(bytes)?)),
    }
}

/// Write the text of the non-null Arrow value at `index`, decoded as `kind`.
pub(crate) fn write_arrow(
    kind: ColumnKind,
    values: &dyn Array,
    index: usize,
    out: &mut impl Write,
) -> Result<(), CellError> {
    let result = match kind {
        ColumnKind::Int2 => write!(out, "{}", primitive::<Int16Type>(values, index)?),
        ColumnKind::Int4 => write!(out, "{}", primitive::<Int32Type>(values, index)?),
        ColumnKind::Int8 => write!(out, "{}", primitive::<Int64Type>(values, index)?),
        ColumnKind::Float4 => write_float(primitive::<Float32Type>(values, index)?, 6, out),
        ColumnKind::Float8 => write_float(primitive::<Float64Type>(values, index)?, 15, out),
        ColumnKind::Bool => out.write_str(bool_text(
            values
                .as_boolean_opt()
                .ok_or(CellError::Format)?
                .value(index),
        )),
        ColumnKind::Bytea => write_hex(
            values
                .as_binary_opt::<i32>()
                .ok_or(CellError::Format)?
                .value(index),
            out,
        ),
        ColumnKind::Uuid => {
            let fixed = values.as_fixed_size_binary_opt().ok_or(CellError::Format)?;
            write_uuid(fixed.value(index), out)
        }
        _ => return write_temporal(kind, values, index, out),
    };
    result.map_err(|_| CellError::Format)
}

fn write_temporal(
    kind: ColumnKind,
    values: &dyn Array,
    index: usize,
    out: &mut impl Write,
) -> Result<(), CellError> {
    let result = match kind {
        ColumnKind::Timestamp | ColumnKind::TimestampTz => {
            let micros = primitive::<TimestampMicrosecondType>(values, index)?;
            write_timestamp(
                micros.saturating_sub(PG_EPOCH_MICROS),
                kind == ColumnKind::TimestampTz,
                out,
            )
        }
        ColumnKind::Date => write_date(
            primitive::<Date32Type>(values, index)?.saturating_sub(PG_EPOCH_DAYS),
            out,
        ),
        ColumnKind::Time => {
            temporal::write_clock(primitive::<Time64MicrosecondType>(values, index)?, out)
        }
        ColumnKind::Interval => {
            Interval::from_month_day_nano(primitive::<IntervalMonthDayNanoType>(values, index)?)
                .write(out)
        }
        ColumnKind::Decimal { scale, .. } => {
            write_decimal(primitive::<Decimal128Type>(values, index)?, scale, out)
        }
        _ => {
            let text = values.as_string_opt::<i32>().ok_or(CellError::Format)?;
            out.write_str(text.value(index))
        }
    };
    result.map_err(|_| CellError::Format)
}

fn primitive<T: ArrowPrimitiveType>(
    values: &dyn Array,
    index: usize,
) -> Result<T::Native, CellError> {
    values
        .as_primitive_opt::<T>()
        .filter(|values| index < values.len())
        .map(|values| values.value(index))
        .ok_or(CellError::Format)
}

fn bool_text(value: bool) -> &'static str {
    if value { "t" } else { "f" }
}

/// `bytea_output = hex`.
fn write_hex(bytes: &[u8], out: &mut impl Write) -> fmt::Result {
    out.write_str("\\x")?;
    bytes.iter().try_for_each(|byte| write!(out, "{byte:02x}"))
}

fn write_uuid(bytes: &[u8], out: &mut impl Write) -> fmt::Result {
    for (index, byte) in bytes.iter().enumerate() {
        if matches!(index, 4 | 6 | 8 | 10) {
            out.write_char('-')?;
        }
        write!(out, "{byte:02x}")?;
    }
    Ok(())
}

/// Float trait for the shortest round-trip text both widths share.
trait Float: Copy + Display + fmt::LowerExp {
    fn is_nan(self) -> bool;
    fn is_infinite(self) -> bool;
    fn is_sign_negative(self) -> bool;
}

impl Float for f32 {
    fn is_nan(self) -> bool {
        self.is_nan()
    }
    fn is_infinite(self) -> bool {
        self.is_infinite()
    }
    fn is_sign_negative(self) -> bool {
        self.is_sign_negative()
    }
}

impl Float for f64 {
    fn is_nan(self) -> bool {
        self.is_nan()
    }
    fn is_infinite(self) -> bool {
        self.is_infinite()
    }
    fn is_sign_negative(self) -> bool {
        self.is_sign_negative()
    }
}

/// Shortest round-trip digits (`extra_float_digits = 1`), in exponent form
/// below 1e-4 or from 10^`digits` up, like `float8out`: `1e+20`, `1.5e-05`.
fn write_float<F: Float>(value: F, digits: i32, out: &mut impl Write) -> fmt::Result {
    if value.is_nan() {
        return out.write_str("NaN");
    }
    if value.is_infinite() {
        return out.write_str(if value.is_sign_negative() {
            "-Infinity"
        } else {
            "Infinity"
        });
    }
    let scientific = format!("{value:e}");
    let (mantissa, exponent) = scientific.split_once('e').unwrap_or((&scientific, "0"));
    let exponent: i32 = exponent.parse().unwrap_or_default();
    if (-4..digits).contains(&exponent) {
        return write!(out, "{value}");
    }
    let sign = if exponent < 0 { '-' } else { '+' };
    write!(out, "{mantissa}e{sign}{:02}", exponent.unsigned_abs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::types::IntervalMonthDayNano;
    use arrow_array::{BooleanArray, Float64Array, IntervalMonthDayNanoArray, StringArray};

    fn wire(kind: ColumnKind, bytes: &[u8]) -> Result<String, CellError> {
        let mut out = String::new();
        write_wire(kind, bytes, &mut out)?;
        Ok(out)
    }

    fn arrow(kind: ColumnKind, values: &dyn Array) -> Result<String, CellError> {
        let mut out = String::new();
        write_arrow(kind, values, 0, &mut out)?;
        Ok(out)
    }

    #[test]
    fn floats_print_like_float8out() {
        let text = |value: f64| {
            let mut out = String::new();
            assert!(write_float(value, 15, &mut out).is_ok());
            out
        };
        assert_eq!(text(1.5), "1.5");
        assert_eq!(text(1e20), "1e+20");
        assert_eq!(text(0.0001), "0.0001");
        assert_eq!(text(1.5e-5), "1.5e-05");
        assert_eq!(text(-0.25), "-0.25");
        assert_eq!(text(f64::NAN), "NaN");
        assert_eq!(text(f64::NEG_INFINITY), "-Infinity");
        assert_eq!(text(123_456_789_012_345.0), "123456789012345");
        assert_eq!(text(1_234_567_890_123_456.0), "1.234567890123456e+15");
    }

    #[test]
    fn wire_values_print_as_postgres_text() -> Result<(), CellError> {
        assert_eq!(wire(ColumnKind::Int2, &(-7_i16).to_be_bytes())?, "-7");
        assert_eq!(wire(ColumnKind::Bool, &[1])?, "t");
        assert_eq!(wire(ColumnKind::Bytea, &[0, 0xab])?, "\\x00ab");
        assert_eq!(wire(ColumnKind::Jsonb, b"\x01{}")?, "{}");
        assert_eq!(
            wire(ColumnKind::Uuid, &[0x12; 16])?,
            "12121212-1212-1212-1212-121212121212"
        );
        assert_eq!(wire(ColumnKind::Date, &0_i32.to_be_bytes())?, "2000-01-01");
        assert_eq!(wire(ColumnKind::Text, "é".as_bytes())?, "é");
        Ok(())
    }

    #[test]
    fn arrow_values_print_as_postgres_text() -> Result<(), CellError> {
        assert_eq!(
            arrow(ColumnKind::Bool, &BooleanArray::from(vec![false]))?,
            "f"
        );
        assert_eq!(
            arrow(ColumnKind::Float8, &Float64Array::from(vec![2.5]))?,
            "2.5"
        );
        let interval = IntervalMonthDayNanoArray::from(vec![IntervalMonthDayNano::new(1, 2, 0)]);
        assert_eq!(arrow(ColumnKind::Interval, &interval)?, "1 mon 2 days");
        assert_eq!(
            arrow(ColumnKind::Inet, &StringArray::from(vec!["::1"]))?,
            "::1"
        );
        let wrong = arrow(ColumnKind::Int4, &StringArray::from(vec!["1"]));
        assert_eq!(wrong, Err(CellError::Format));
        Ok(())
    }
}
