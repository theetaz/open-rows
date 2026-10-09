// SPDX-License-Identifier: Apache-2.0
//! Canonical text of one Arrow cell, for comparing decoded values with the
//! expected ones independently of the Arrow type a driver chose.

use std::fmt::Write as _;

use arrow_array::cast::AsArray;
use arrow_array::types::{
    Date32Type, Decimal128Type, Float32Type, Float64Type, Int8Type, Int16Type, Int32Type,
    Int64Type, IntervalMonthDayNanoType, Time64MicrosecondType, UInt64Type,
};
use arrow_array::{Array, ArrowPrimitiveType, PrimitiveArray};
use arrow_schema::DataType;
use arrow_schema::{IntervalUnit, TimeUnit};
use openrows_driver_api::text::{IntervalParts, write_array_item, write_clock};

use crate::Failure;

/// Lower-case hex of `bytes`.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

fn primitive<T: ArrowPrimitiveType>(array: &dyn Array, row: usize) -> String
where
    T::Native: ToString,
{
    let values: &PrimitiveArray<T> = array.as_primitive();
    values.value(row).to_string()
}

/// The cell's value as text: integers and floats in Rust's shortest form,
/// booleans `true`/`false`, binary as lower-case hex, decimals with their
/// scale, dates as `YYYY-MM-DD`, times as `HH:MM:SS[.ffffff]`, intervals and
/// lists as `PostgreSQL` prints them; `None` for NULL.
///
/// # Errors
/// [`Failure::Contract`] for an Arrow type no driver should produce.
pub(crate) fn text(array: &dyn Array, row: usize) -> Result<Option<String>, Failure> {
    if array.is_null(row) || array.data_type() == &DataType::Null {
        return Ok(None);
    }
    let value = match array.data_type() {
        DataType::Int8 => primitive::<Int8Type>(array, row),
        DataType::Int16 => primitive::<Int16Type>(array, row),
        DataType::Int32 => primitive::<Int32Type>(array, row),
        DataType::Int64 => primitive::<Int64Type>(array, row),
        DataType::UInt64 => primitive::<UInt64Type>(array, row),
        DataType::Float32 => primitive::<Float32Type>(array, row),
        DataType::Float64 => primitive::<Float64Type>(array, row),
        DataType::Boolean => array.as_boolean().value(row).to_string(),
        DataType::Utf8 => array.as_string::<i32>().value(row).to_owned(),
        DataType::Binary => hex(array.as_binary::<i32>().value(row)),
        DataType::FixedSizeBinary(_) => hex(array.as_fixed_size_binary().value(row)),
        DataType::Decimal128(..) => array.as_primitive::<Decimal128Type>().value_as_string(row),
        DataType::Date32 => array
            .as_primitive::<Date32Type>()
            .value_as_date(row)
            .map(|date| date.to_string())
            .unwrap_or_default(),
        DataType::Time64(TimeUnit::Microsecond) => {
            let micros = array.as_primitive::<Time64MicrosecondType>().value(row);
            let mut out = String::new();
            let _ = write_clock(micros.unsigned_abs(), &mut out);
            out
        }
        DataType::Interval(IntervalUnit::MonthDayNano) => {
            let value = array.as_primitive::<IntervalMonthDayNanoType>().value(row);
            let parts = IntervalParts {
                months: value.months,
                days: value.days,
                micros: value.nanoseconds / 1_000,
            };
            let mut out = String::new();
            let _ = parts.write(&mut out);
            out
        }
        DataType::List(_) => list(array, row)?,
        other => {
            return Err(Failure::Contract(format!(
                "no text form for Arrow type {other}"
            )));
        }
    };
    Ok(Some(value))
}

/// `{1,NULL,"a b"}`: each item's text quoted like `array_out`.
fn list(array: &dyn Array, row: usize) -> Result<String, Failure> {
    let items = array.as_list::<i32>().value(row);
    let mut out = String::from("{");
    for index in 0..items.len() {
        if index > 0 {
            out.push(',');
        }
        match text(items.as_ref(), index)? {
            Some(item) => {
                let _ = write_array_item(&item, &mut out);
            }
            None => out.push_str("NULL"),
        }
    }
    out.push('}');
    Ok(out)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow_array::{ArrayRef, BinaryArray, Date32Array, Decimal128Array, Int64Array};

    use super::*;

    #[test]
    fn cells_render_canonically() -> Result<(), Box<dyn std::error::Error>> {
        let ints: ArrayRef = Arc::new(Int64Array::from(vec![Some(-7), None]));
        assert_eq!(text(&ints, 0)?, Some("-7".to_owned()));
        assert_eq!(text(&ints, 1)?, None);
        let bytes = BinaryArray::from(vec![&[0_u8, 255, 16][..]]);
        assert_eq!(text(&bytes, 0)?, Some("00ff10".to_owned()));
        let decimal = Decimal128Array::from(vec![1250]).with_precision_and_scale(10, 2)?;
        assert_eq!(text(&decimal, 0)?, Some("12.50".to_owned()));
        let date = Date32Array::from(vec![19_849]);
        assert_eq!(text(&date, 0)?, Some("2024-05-06".to_owned()));
        assert_eq!(text(&arrow_array::NullArray::new(1), 0)?, None);
        let time = arrow_array::Time64MicrosecondArray::from(vec![3_723_500_000]);
        assert_eq!(text(&time, 0)?, Some("01:02:03.5".to_owned()));
        let interval = arrow_array::IntervalMonthDayNanoArray::from(vec![
            arrow_array::types::IntervalMonthDayNano::new(14, 3, 0),
        ]);
        assert_eq!(text(&interval, 0)?, Some("1 year 2 mons 3 days".to_owned()));
        let list =
            arrow_array::ListArray::from_iter_primitive::<Int32Type, _, _>(vec![Some(vec![
                Some(1),
                None,
            ])]);
        assert_eq!(text(&list, 0)?, Some("{1,NULL}".to_owned()));
        let time = arrow_array::Time32SecondArray::from(vec![1]);
        assert!(matches!(text(&time, 0), Err(Failure::Contract(_))));
        Ok(())
    }
}
