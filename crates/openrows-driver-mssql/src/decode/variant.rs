// SPDX-License-Identifier: Apache-2.0
//! `sql_variant` values (MS-TDS 2.2.5.5.4): a base type, its properties
//! (precision and scale, a collation, a maximum length) and the value,
//! rendered as the base value's text.

use super::{CellError, Kind, text};
use crate::tds::collation::Collation;

fn malformed() -> CellError {
    CellError::Malformed("malformed sql_variant value".to_owned())
}

/// The base value of a variant as text.
///
/// # Errors
/// [`CellError::Malformed`] for a truncated value or an unknown base type.
pub(crate) fn render(bytes: &[u8]) -> Result<String, CellError> {
    let [base, props_len, rest @ ..] = bytes else {
        return Err(malformed());
    };
    let (props, value) = rest
        .split_at_checked(usize::from(*props_len))
        .ok_or_else(malformed)?;
    text::render(base_kind(*base, props)?, value)
}

fn base_kind(base: u8, props: &[u8]) -> Result<Kind, CellError> {
    let scale = || props.first().copied().ok_or_else(malformed);
    Ok(match base {
        0x30 => Kind::TinyInt,
        0x32 => Kind::Bit,
        0x34 => Kind::SmallInt,
        0x38 => Kind::Int,
        0x7F => Kind::BigInt,
        0x3B => Kind::Real,
        0x3E => Kind::Float,
        0x3C => Kind::Money,
        0x7A => Kind::SmallMoney,
        0x3D => Kind::DateTime,
        0x3A => Kind::SmallDateTime,
        0x24 => Kind::Guid,
        0x28 => Kind::Date,
        0x29 => Kind::Time { scale: scale()? },
        0x2A => Kind::DateTime2 { scale: scale()? },
        0x2B => Kind::DateTimeOffset { scale: scale()? },
        0x37 | 0x3F | 0x6A | 0x6C => match props {
            [precision, scale, ..] => Kind::Decimal {
                precision: *precision,
                scale: *scale,
            },
            _ => return Err(malformed()),
        },
        0xA5 | 0xAD => Kind::Binary,
        0xA7 | 0xAF => Kind::Char(Collation::from_bytes(props)),
        0xE7 | 0xEF => Kind::Utf16,
        _ => return Err(malformed()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tds::cursor::utf16_bytes;

    #[test]
    fn base_values_render_as_text() -> Result<(), CellError> {
        let mut int = vec![0x38, 0];
        int.extend(42_i32.to_le_bytes());
        assert_eq!(render(&int)?, "42");
        let mut decimal = vec![0x6A, 2, 10, 2, 1];
        decimal.extend(1250_u32.to_le_bytes());
        assert_eq!(render(&decimal)?, "12.50");
        let mut nvarchar = vec![0xE7, 7, 9, 4, 0xD0, 0, 52, 0x10, 0];
        nvarchar.extend(utf16_bytes("x"));
        assert_eq!(render(&nvarchar)?, "x");
        assert!(render(&[0x38]).is_err());
        assert!(render(&[0x99, 0]).is_err());
        Ok(())
    }
}
