// SPDX-License-Identifier: Apache-2.0
//! Row payloads as text-form values: text-protocol rows are length-encoded
//! strings already; binary-protocol (prepared statement) rows are decoded
//! with `mysql_common` and rendered the way the server renders text, so both
//! protocols feed the same Arrow decoder and the same edit snapshots.
use mysql_common::constants::{ColumnFlags, ColumnType};
use mysql_common::io::ParseBuf;
use mysql_common::packets::{Column, NullBitmap};
use mysql_common::value::{BinValue, ServerSide, Value, ValueDeserializer};
use openrows_driver_api::DriverError;

/// One text-protocol row: `None` is SQL NULL.
///
/// # Errors
/// `Protocol` when the packet is shorter than its length prefixes say.
pub(crate) fn text_row(packet: &[u8], columns: usize) -> Result<Vec<Option<Vec<u8>>>, DriverError> {
    let mut rest = packet;
    let mut values = Vec::with_capacity(columns);
    for _ in 0..columns {
        if rest.first() == Some(&0xfb) {
            values.push(None);
            rest = rest.get(1..).unwrap_or_default();
            continue;
        }
        let (length, after) = lenenc(rest)?;
        let length = usize::try_from(length).map_err(|_| short())?;
        let value = after.get(..length).ok_or_else(short)?;
        values.push(Some(value.to_vec()));
        rest = after.get(length..).unwrap_or_default();
    }
    Ok(values)
}

/// A length-encoded integer and the bytes after it.
pub(crate) fn lenenc(bytes: &[u8]) -> Result<(u64, &[u8]), DriverError> {
    let (&first, rest) = bytes.split_first().ok_or_else(short)?;
    let width = match first {
        0xfc => 2,
        0xfd => 3,
        0xfe => 8,
        0xfb | 0xff => return Err(short()),
        small => return Ok((u64::from(small), rest)),
    };
    let digits = rest.get(..width).ok_or_else(short)?;
    let value = digits
        .iter()
        .rev()
        .fold(0_u64, |value, byte| value << 8 | u64::from(*byte));
    Ok((value, rest.get(width..).unwrap_or_default()))
}

fn short() -> DriverError {
    DriverError::Protocol("row packet is shorter than its length prefixes".to_owned())
}

/// One binary-protocol row, each value in its text form.
///
/// # Errors
/// `Protocol` for a malformed row.
pub(crate) fn binary_row(
    packet: &[u8],
    columns: &[Column],
) -> Result<Vec<Option<Vec<u8>>>, DriverError> {
    let bitmap_len = NullBitmap::<ServerSide>::bitmap_len(columns.len());
    let bitmap = packet.get(1..=bitmap_len).ok_or_else(short)?;
    let nulls = NullBitmap::<ServerSide, &[u8]>::from_bytes(bitmap);
    let mut buffer = ParseBuf(packet.get(1 + bitmap_len..).unwrap_or_default());
    let mut values = Vec::with_capacity(columns.len());
    for (index, column) in columns.iter().enumerate() {
        if nulls.is_null(index) {
            values.push(None);
            continue;
        }
        let context = (column.column_type(), column.flags());
        let value: ValueDeserializer<BinValue> = buffer
            .parse(context)
            .map_err(|error| DriverError::Protocol(format!("binary row: {error}")))?;
        values.push(text_form(value.0, column));
    }
    Ok(values)
}

/// The text the server would send for `value` in `column`.
fn text_form(value: Value, column: &Column) -> Option<Vec<u8>> {
    let text = match value {
        Value::NULL => return None,
        Value::Bytes(bytes) => return Some(bytes),
        Value::Int(integer) => integer.to_string(),
        Value::UInt(integer) => integer.to_string(),
        Value::Float(real) => real.to_string(),
        Value::Double(real) => real.to_string(),
        Value::Date(year, month, day, hour, minute, second, micros) => {
            let date = format!("{year:04}-{month:02}-{day:02}");
            if column.column_type() == ColumnType::MYSQL_TYPE_DATE {
                date
            } else {
                format!(
                    "{date} {hour:02}:{minute:02}:{second:02}{}",
                    fraction(micros, column.decimals())
                )
            }
        }
        Value::Time(negative, days, hours, minutes, seconds, micros) => {
            let sign = if negative { "-" } else { "" };
            let hours = u64::from(days) * 24 + u64::from(hours);
            format!(
                "{sign}{hours:02}:{minutes:02}:{seconds:02}{}",
                fraction(micros, column.decimals())
            )
        }
    };
    Some(text.into_bytes())
}

/// `.ffffff` cut to the column's fractional-second precision.
fn fraction(micros: u32, decimals: u8) -> String {
    let digits = usize::from(decimals.min(6));
    if digits == 0 {
        return String::new();
    }
    let full = format!("{micros:06}");
    format!(".{}", full.get(..digits).unwrap_or(&full))
}

/// True for columns the server flags as unsigned.
pub(crate) fn unsigned(column: &Column) -> bool {
    column.flags().contains(ColumnFlags::UNSIGNED_FLAG)
}

#[cfg(test)]
mod tests;
