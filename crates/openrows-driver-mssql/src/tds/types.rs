// SPDX-License-Identifier: Apache-2.0
//! `TYPE_INFO` (MS-TDS 2.2.5.4) and how each type's values are laid out in
//! a row: fixed width, a 1-, 2- or 4-byte length prefix, partially
//! length-prefixed (`PLP`) chunks for `(max)`, `xml` and CLR types, or a
//! text pointer for `text`, `ntext` and `image`.

use std::borrow::Cow;

use super::collation::Collation;
use super::cursor::{Cursor, Parsed, invalid};

/// What a column holds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SqlType {
    Null,
    TinyInt,
    SmallInt,
    Int,
    BigInt,
    Bit,
    Real,
    Float,
    SmallMoney,
    Money,
    SmallDateTime,
    DateTime,
    Guid,
    /// `decimal`/`numeric(p, s)`; `numeric` names the source type.
    Decimal {
        precision: u8,
        scale: u8,
        numeric: bool,
    },
    Date,
    Time {
        scale: u8,
    },
    DateTime2 {
        scale: u8,
    },
    DateTimeOffset {
        scale: u8,
    },
    /// `char`, `varchar` (single-byte or UTF-8 by collation).
    Char {
        collation: Collation,
        varying: bool,
    },
    /// `nchar`, `nvarchar` (UTF-16).
    NChar {
        varying: bool,
    },
    Binary {
        varying: bool,
    },
    Text {
        collation: Collation,
    },
    NText,
    Image,
    Xml,
    /// A CLR type (`geography`, `geometry`, `hierarchyid`, user types).
    Udt {
        name: String,
    },
    Variant,
}

/// How a value of the type is laid out in a row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Layout {
    Fixed(usize),
    /// A one-byte length; 0 is NULL.
    ByteLen,
    /// A two-byte length; `0xFFFF` is NULL.
    UShortLen,
    /// Partially length-prefixed chunks.
    Plp,
    /// A text pointer, a timestamp and a four-byte length.
    TextPtr,
    /// A four-byte length; 0 is NULL (`sql_variant`).
    LongLen,
}

/// A column's type and layout.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TypeInfo {
    pub(crate) sql: SqlType,
    pub(crate) layout: Layout,
}

impl TypeInfo {
    fn new(sql: SqlType, layout: Layout) -> Self {
        Self { sql, layout }
    }
}

/// Parse a `TYPE_INFO`.
pub(crate) fn type_info(cursor: &mut Cursor<'_>) -> Parsed<TypeInfo> {
    let code = cursor.u8()?;
    if let Some(info) = fixed(code) {
        return Ok(info);
    }
    match code {
        0x26 | 0x68 | 0x6D | 0x6E | 0x6F | 0x24 => byte_len(code, cursor.u8()?),
        0x37 | 0x3F | 0x6A | 0x6C => {
            cursor.u8()?;
            let (precision, scale) = (cursor.u8()?, cursor.u8()?);
            let numeric = matches!(code, 0x3F | 0x6C);
            let sql = SqlType::Decimal {
                precision,
                scale,
                numeric,
            };
            Ok(TypeInfo::new(sql, Layout::ByteLen))
        }
        0x28 => Ok(TypeInfo::new(SqlType::Date, Layout::ByteLen)),
        0x29..=0x2B => scaled(code, cursor.u8()?),
        0xA7 | 0xAF | 0xE7 | 0xEF | 0xA5 | 0xAD => short_len(code, cursor),
        0x23 | 0x63 | 0x22 | 0x62 => long_len(code, cursor),
        0xF1 => xml(cursor),
        0xF0 => udt(cursor),
        other => invalid(format!("unsupported TDS data type 0x{other:02X}")),
    }
}

fn fixed(code: u8) -> Option<TypeInfo> {
    let (sql, width) = match code {
        0x1F => (SqlType::Null, 0),
        0x30 => (SqlType::TinyInt, 1),
        0x32 => (SqlType::Bit, 1),
        0x34 => (SqlType::SmallInt, 2),
        0x38 => (SqlType::Int, 4),
        0x7F => (SqlType::BigInt, 8),
        0x3A => (SqlType::SmallDateTime, 4),
        0x3D => (SqlType::DateTime, 8),
        0x3B => (SqlType::Real, 4),
        0x3E => (SqlType::Float, 8),
        0x3C => (SqlType::Money, 8),
        0x7A => (SqlType::SmallMoney, 4),
        _ => return None,
    };
    Some(TypeInfo::new(sql, Layout::Fixed(width)))
}

/// Nullable fixed types: the declared length picks the type.
fn byte_len(code: u8, len: u8) -> Parsed<TypeInfo> {
    let sql = match (code, len) {
        (0x26, 1) => SqlType::TinyInt,
        (0x26, 2) => SqlType::SmallInt,
        (0x26, 4) => SqlType::Int,
        (0x26, 8) => SqlType::BigInt,
        (0x68, 1) => SqlType::Bit,
        (0x6D, 4) => SqlType::Real,
        (0x6D, 8) => SqlType::Float,
        (0x6E, 4) => SqlType::SmallMoney,
        (0x6E, 8) => SqlType::Money,
        (0x6F, 4) => SqlType::SmallDateTime,
        (0x6F, 8) => SqlType::DateTime,
        (0x24, 16) => SqlType::Guid,
        _ => return invalid(format!("TDS type 0x{code:02X} with length {len}")),
    };
    Ok(TypeInfo::new(sql, Layout::ByteLen))
}

fn scaled(code: u8, scale: u8) -> Parsed<TypeInfo> {
    if scale > 7 {
        return invalid(format!("fractional-second scale {scale}"));
    }
    let sql = match code {
        0x29 => SqlType::Time { scale },
        0x2A => SqlType::DateTime2 { scale },
        _ => SqlType::DateTimeOffset { scale },
    };
    Ok(TypeInfo::new(sql, Layout::ByteLen))
}

/// `(n)varchar`, `(n)char`, `(var)binary`: a two-byte maximum length,
/// `0xFFFF` for `(max)`, then a collation for character types.
fn short_len(code: u8, cursor: &mut Cursor<'_>) -> Parsed<TypeInfo> {
    let max = cursor.u16()? == u16::MAX;
    let varying = matches!(code, 0xA7 | 0xE7 | 0xA5);
    let sql = match code {
        0xA7 | 0xAF => SqlType::Char {
            collation: Collation::read(cursor)?,
            varying,
        },
        0xE7 | 0xEF => {
            Collation::read(cursor)?;
            SqlType::NChar { varying }
        }
        _ => SqlType::Binary { varying },
    };
    let layout = if max { Layout::Plp } else { Layout::UShortLen };
    Ok(TypeInfo::new(sql, layout))
}

fn long_len(code: u8, cursor: &mut Cursor<'_>) -> Parsed<TypeInfo> {
    cursor.u32()?;
    let sql = match code {
        0x23 => SqlType::Text {
            collation: Collation::read(cursor)?,
        },
        0x63 => {
            Collation::read(cursor)?;
            SqlType::NText
        }
        0x22 => SqlType::Image,
        _ => return Ok(TypeInfo::new(SqlType::Variant, Layout::LongLen)),
    };
    Ok(TypeInfo::new(sql, Layout::TextPtr))
}

fn xml(cursor: &mut Cursor<'_>) -> Parsed<TypeInfo> {
    if cursor.u8()? != 0 {
        cursor.b_varchar()?;
        cursor.b_varchar()?;
        cursor.us_varchar()?;
    }
    Ok(TypeInfo::new(SqlType::Xml, Layout::Plp))
}

fn udt(cursor: &mut Cursor<'_>) -> Parsed<TypeInfo> {
    cursor.u16()?;
    cursor.b_varchar()?;
    cursor.b_varchar()?;
    let name = cursor.b_varchar()?;
    cursor.us_varchar()?;
    Ok(TypeInfo::new(SqlType::Udt { name }, Layout::Plp))
}

/// One value's bytes (without its length prefix); `None` is NULL.
pub(crate) fn value<'a>(layout: Layout, cursor: &mut Cursor<'a>) -> Parsed<Option<Cow<'a, [u8]>>> {
    let borrowed = |bytes: &'a [u8]| Some(Cow::Borrowed(bytes));
    match layout {
        Layout::Fixed(width) => Ok(borrowed(cursor.take(width)?)),
        Layout::ByteLen => match cursor.u8()? {
            0 => Ok(None),
            len => Ok(borrowed(cursor.take(usize::from(len))?)),
        },
        Layout::UShortLen => match cursor.u16()? {
            u16::MAX => Ok(None),
            len => Ok(borrowed(cursor.take(usize::from(len))?)),
        },
        Layout::Plp => plp(cursor),
        Layout::TextPtr => match cursor.u8()? {
            0 => Ok(None),
            pointer => {
                cursor.skip(usize::from(pointer) + 8)?;
                let len = length(cursor.u32()?)?;
                Ok(borrowed(cursor.take(len)?))
            }
        },
        Layout::LongLen => match cursor.u32()? {
            0 => Ok(None),
            len => Ok(borrowed(cursor.take(length(len)?)?)),
        },
    }
}

fn length(value: u32) -> Parsed<usize> {
    usize::try_from(value).or_else(|_| invalid("value length overflows"))
}

/// `PLP_BODY`: a total length (all ones is NULL), then chunks until a zero
/// chunk. A single chunk is borrowed; several are joined.
fn plp<'a>(cursor: &mut Cursor<'a>) -> Parsed<Option<Cow<'a, [u8]>>> {
    if cursor.u64()? == u64::MAX {
        return Ok(None);
    }
    let mut parts: Vec<&'a [u8]> = Vec::new();
    loop {
        let len = length(cursor.u32()?)?;
        if len == 0 {
            break;
        }
        parts.push(cursor.take(len)?);
    }
    Ok(Some(match parts.as_slice() {
        [] => Cow::Borrowed(&[][..]),
        [one] => Cow::Borrowed(*one),
        many => Cow::Owned(many.concat()),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(bytes: &[u8]) -> Option<TypeInfo> {
        type_info(&mut Cursor::new(bytes)).ok()
    }

    #[test]
    fn type_infos_name_their_types() {
        assert_eq!(parse(&[0x26, 4]).map(|t| t.sql), Some(SqlType::Int));
        assert_eq!(
            parse(&[0x6C, 9, 10, 2]).map(|t| t.sql),
            Some(SqlType::Decimal {
                precision: 10,
                scale: 2,
                numeric: true
            })
        );
        let varchar = parse(&[0xA7, 0xFF, 0xFF, 9, 4, 0xD0, 0, 52]);
        assert_eq!(varchar.map(|t| t.layout), Some(Layout::Plp));
        assert_eq!(
            parse(&[0x2A, 7]).map(|t| t.sql),
            Some(SqlType::DateTime2 { scale: 7 })
        );
        assert!(parse(&[0x2A, 8]).is_none());
        assert!(parse(&[0x99]).is_none());
        assert!(parse(&[0x26, 3]).is_none());
    }

    #[test]
    fn values_follow_their_layout() {
        let mut cursor = Cursor::new(&[0, 2, 0xAB, 0xCD, 0xFF, 0xFF]);
        assert!(matches!(value(Layout::ByteLen, &mut cursor), Ok(None)));
        let bytes = value(Layout::ByteLen, &mut cursor).ok().flatten();
        assert_eq!(bytes.as_deref(), Some(&[0xAB, 0xCD][..]));
        assert!(matches!(value(Layout::UShortLen, &mut cursor), Ok(None)));
        let mut plp_bytes = vec![0xFE; 8];
        plp_bytes.extend_from_slice(&[2, 0, 0, 0, b'a', b'b', 1, 0, 0, 0, b'c', 0, 0, 0, 0]);
        let joined = value(Layout::Plp, &mut Cursor::new(&plp_bytes))
            .ok()
            .flatten();
        assert_eq!(joined.as_deref(), Some(&b"abc"[..]));
        assert!(matches!(
            value(Layout::Plp, &mut Cursor::new(&plp_bytes[..12])),
            Err(crate::tds::cursor::Short::Incomplete)
        ));
        let null = [0xFF; 8];
        assert!(matches!(
            value(Layout::Plp, &mut Cursor::new(&null)),
            Ok(None)
        ));
    }
}
