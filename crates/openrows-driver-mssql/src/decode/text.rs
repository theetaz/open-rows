// SPDX-License-Identifier: Apache-2.0
//! Values as text, as SQL Server prints them: for catalog and internal
//! queries, `sql_variant` values, `datetimeoffset` and the tagged-text
//! fallback.

use std::fmt::Write as _;

use super::{CellError, Kind, number, temporal, variant};

/// Append UTF-16LE text, without a leading byte-order mark (`xml` values
/// carry one); unpaired surrogates become U+FFFD.
pub(crate) fn push_utf16(bytes: &[u8], out: &mut String) {
    let bytes = bytes.strip_prefix(&[0xFF, 0xFE]).unwrap_or(bytes);
    let units = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]));
    out.extend(char::decode_utf16(units).map(|unit| unit.unwrap_or(char::REPLACEMENT_CHARACTER)));
}

/// The text of one value of `kind`.
///
/// # Errors
/// [`CellError::Malformed`] when the bytes do not fit the type.
pub(crate) fn render(kind: Kind, bytes: &[u8]) -> Result<String, CellError> {
    let mut out = String::new();
    render_into(kind, bytes, &mut out)?;
    Ok(out)
}

/// Append the text of one value of `kind` to `out`.
///
/// # Errors
/// [`CellError::Malformed`] when the bytes do not fit the type.
pub(crate) fn render_into(kind: Kind, bytes: &[u8], out: &mut String) -> Result<(), CellError> {
    match kind {
        Kind::TinyInt => write_display(number::tinyint(bytes)?, out),
        Kind::SmallInt => write_display(number::smallint(bytes)?, out),
        Kind::Int => write_display(number::int(bytes)?, out),
        Kind::BigInt => write_display(number::bigint(bytes)?, out),
        Kind::Bit => out.push(if number::bit(bytes)? { '1' } else { '0' }),
        Kind::Real => write_display(number::real(bytes)?, out),
        Kind::Float => write_display(number::float(bytes)?, out),
        Kind::Decimal { scale, .. } => number::write_scaled(number::decimal(bytes)?, scale, out),
        Kind::SmallMoney | Kind::Money => number::write_scaled(number::money(bytes)?, 4, out),
        Kind::Guid => number::write_guid(&number::guid(bytes)?, out),
        Kind::Binary => write_hex(bytes, out),
        Kind::Char(collation) => {
            let (text, _) = collation.encoding().decode_without_bom_handling(bytes);
            out.push_str(&text);
        }
        Kind::Utf16 | Kind::Null => push_utf16(bytes, out),
        Kind::Variant => out.push_str(&variant::render(bytes)?),
        temporal_kind => render_temporal(temporal_kind, bytes, out)?,
    }
    Ok(())
}

fn write_display(value: impl std::fmt::Display, out: &mut String) {
    let _ = write!(out, "{value}");
}

fn render_temporal(kind: Kind, bytes: &[u8], out: &mut String) -> Result<(), CellError> {
    match kind {
        Kind::Date => temporal::write_date(temporal::date_days(bytes)?, out),
        Kind::Time { scale } => {
            temporal::write_clock(temporal::time_nanos(bytes, scale)?, scale, out);
        }
        Kind::DateTime2 { scale } => {
            let micros = temporal::datetime2_micros(bytes, scale)?;
            let nanos = if scale > 6 {
                datetime2_extra_nanos(bytes)?
            } else {
                0
            };
            temporal::write_timestamp(micros, (nanos, scale), out);
        }
        Kind::DateTime => temporal::write_timestamp(temporal::datetime_micros(bytes)?, (0, 3), out),
        Kind::SmallDateTime => {
            temporal::write_timestamp(temporal::smalldatetime_micros(bytes)?, (0, 0), out);
        }
        Kind::DateTimeOffset { scale } => {
            out.push_str(&temporal::datetimeoffset_text(bytes, scale)?);
        }
        _ => {}
    }
    Ok(())
}

/// The sub-microsecond nanoseconds (the 100 ns digit) of a
/// `datetime2(7)` value, whose time of day is its first five bytes.
fn datetime2_extra_nanos(bytes: &[u8]) -> Result<i64, CellError> {
    let time = bytes
        .get(..5)
        .ok_or_else(|| CellError::Malformed("malformed datetime2 value".to_owned()))?;
    Ok(temporal::time_nanos(time, 7)? % 1_000)
}

/// `0x` and upper-case hex digits.
pub(crate) fn write_hex(bytes: &[u8], out: &mut String) {
    out.reserve(2 + bytes.len() * 2);
    out.push_str("0x");
    for byte in bytes {
        let _ = write!(out, "{byte:02X}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tds::collation::Collation;
    use crate::tds::cursor::utf16_bytes;

    #[test]
    fn values_print_like_the_server() -> Result<(), CellError> {
        assert_eq!(render(Kind::Int, &(-7_i32).to_le_bytes())?, "-7");
        assert_eq!(render(Kind::Bit, &[1])?, "1");
        assert_eq!(render(Kind::Float, &0.1_f64.to_le_bytes())?, "0.1");
        assert_eq!(
            render(Kind::Money, &[0, 0, 0, 0, 0x10, 0x27, 0, 0])?,
            "1.0000"
        );
        assert_eq!(render(Kind::Binary, &[0, 0xAB])?, "0x00AB");
        assert_eq!(render(Kind::Utf16, &utf16_bytes("h\u{e9}"))?, "h\u{e9}");
        let mut xml = vec![0xFF, 0xFE];
        xml.extend(utf16_bytes("<a/>"));
        assert_eq!(render(Kind::Utf16, &xml)?, "<a/>");
        assert_eq!(render(Kind::Char(Collation::default()), b"abc")?, "abc");
        Ok(())
    }

    #[test]
    fn datetime2_keeps_all_seven_digits() -> Result<(), CellError> {
        let mut bytes = 1_234_567_u64.to_le_bytes()[..5].to_vec();
        bytes.extend(&719_162_u32.to_le_bytes()[..3]);
        let text = render(Kind::DateTime2 { scale: 7 }, &bytes)?;
        assert_eq!(text, "1970-01-01 00:00:00.1234567");
        let date = render(Kind::Date, &719_162_u32.to_le_bytes()[..3])?;
        assert_eq!(date, "1970-01-01");
        Ok(())
    }
}
