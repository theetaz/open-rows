// SPDX-License-Identifier: Apache-2.0
//! Collations (MS-TDS 2.2.5.1.2) and the code page `char`, `varchar` and
//! `text` values are stored in. A collation is a 20-bit Windows locale id,
//! flags (bit 26: UTF-8, SQL Server 2019+), a version and, for the legacy
//! `SQL_*` collations, a sort id that names the code page directly.
//!
//! The code pages follow the locale tables of Microsoft's own TDS drivers.
//! The SQL Server 2000 era IBM 437/850 sort orders have no decoder in
//! `encoding_rs` and decode as Windows-1252 (identical below 0x80).

use encoding_rs::{
    BIG5, EUC_KR, Encoding, GB18030, SHIFT_JIS, UTF_8, WINDOWS_874, WINDOWS_1250, WINDOWS_1251,
    WINDOWS_1252, WINDOWS_1253, WINDOWS_1254, WINDOWS_1255, WINDOWS_1256, WINDOWS_1257,
    WINDOWS_1258,
};

use super::cursor::{Cursor, Parsed};

/// The UTF-8 flag of the collation word.
const UTF8_FLAG: u32 = 1 << 26;

/// A five-byte collation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Collation {
    info: u32,
    sort_id: u8,
}

impl Collation {
    pub(crate) fn read(cursor: &mut Cursor<'_>) -> Parsed<Self> {
        let bytes = cursor.take(5)?;
        Ok(Self::from_bytes(bytes))
    }

    /// From the five wire bytes; shorter input is the default collation.
    pub(crate) fn from_bytes(bytes: &[u8]) -> Self {
        match bytes {
            [a, b, c, d, sort_id, ..] => Self {
                info: u32::from_le_bytes([*a, *b, *c, *d]),
                sort_id: *sort_id,
            },
            _ => Self::default(),
        }
    }

    /// The five wire bytes.
    pub(crate) fn to_bytes(self) -> [u8; 5] {
        let [a, b, c, d] = self.info.to_le_bytes();
        [a, b, c, d, self.sort_id]
    }

    /// The code page of single-byte character data.
    pub(crate) fn encoding(self) -> &'static Encoding {
        if self.info & UTF8_FLAG != 0 {
            return UTF_8;
        }
        if self.sort_id != 0
            && let Some(encoding) = by_sort_id(self.sort_id)
        {
            return encoding;
        }
        by_locale(self.info & 0xF_FFFF)
    }
}

/// Locales whose script differs from their language's usual code page.
fn by_locale(lcid: u32) -> &'static Encoding {
    match lcid {
        0x0404 | 0x0C04 | 0x1404 => BIG5,
        0x0804 | 0x1004 => GB18030,
        0x082C | 0x0843 | 0x0C1A | 0x1C1A | 0x201A => WINDOWS_1251,
        _ => by_language(lcid & 0x3FF),
    }
}

/// The code page of a primary language id.
fn by_language(language: u32) -> &'static Encoding {
    match language {
        0x01 | 0x20 | 0x29 | 0x80 | 0x8C => WINDOWS_1256,
        0x02 | 0x19 | 0x22 | 0x23 | 0x28 | 0x2F | 0x3F | 0x40 | 0x44 | 0x50 | 0x6D | 0x85 => {
            WINDOWS_1251
        }
        0x05 | 0x0E | 0x15 | 0x18 | 0x1A | 0x1B | 0x1C | 0x24 | 0x42 => WINDOWS_1250,
        0x08 => WINDOWS_1253,
        0x0D => WINDOWS_1255,
        0x11 => SHIFT_JIS,
        0x12 => EUC_KR,
        0x1E => WINDOWS_874,
        0x1F | 0x2C | 0x43 => WINDOWS_1254,
        0x25..=0x27 => WINDOWS_1257,
        0x2A => WINDOWS_1258,
        _ => WINDOWS_1252,
    }
}

/// The code page of a legacy `SQL_*` sort order.
fn by_sort_id(sort_id: u8) -> Option<&'static Encoding> {
    Some(match sort_id {
        30..=61 | 71..=75 | 183..=186 => WINDOWS_1252,
        80..=98 => WINDOWS_1250,
        104..=108 => WINDOWS_1251,
        112..=114 | 120..=122 | 124 => WINDOWS_1253,
        128..=130 => WINDOWS_1254,
        136..=138 => WINDOWS_1255,
        144..=146 => WINDOWS_1256,
        152..=160 => WINDOWS_1257,
        192 | 193 | 200 => SHIFT_JIS,
        194 | 195 => EUC_KR,
        196 | 197 | 202 => BIG5,
        198 | 199 | 203 => GB18030,
        204..=206 => WINDOWS_874,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collation(lcid: u32, sort_id: u8) -> Collation {
        Collation {
            info: lcid,
            sort_id,
        }
    }

    #[test]
    fn code_pages_follow_locale_sort_order_and_utf8_flag() {
        assert_eq!(collation(0x0409, 52).encoding(), WINDOWS_1252);
        assert_eq!(collation(0x0419, 0).encoding(), WINDOWS_1251);
        assert_eq!(collation(0x0411, 0).encoding(), SHIFT_JIS);
        assert_eq!(collation(0x0804, 0).encoding(), GB18030);
        assert_eq!(collation(0x0C1A, 0).encoding(), WINDOWS_1251);
        assert_eq!(collation(0x081A, 0).encoding(), WINDOWS_1250);
        assert_eq!(collation(0x0409, 104).encoding(), WINDOWS_1251);
        assert_eq!(collation(0x0409 | UTF8_FLAG, 0).encoding(), UTF_8);
        assert_eq!(collation(0x0405, 250).encoding(), WINDOWS_1250);
    }

    #[test]
    fn character_data_decodes_by_code_page_and_bytes_round_trip() {
        let decode = |collation: Collation, bytes: &[u8]| {
            collation
                .encoding()
                .decode_without_bom_handling(bytes)
                .0
                .into_owned()
        };
        assert_eq!(
            decode(collation(0x0409, 52), &[0x63, 0x61, 0x66, 0xE9]),
            "caf\u{e9}"
        );
        assert_eq!(
            decode(collation(0x0419, 0), &[0xCF, 0xF0]),
            "\u{41f}\u{440}"
        );
        let latin = Collation::from_bytes(&[0x09, 0x04, 0xD0, 0x00, 0x34]);
        assert_eq!(latin.to_bytes(), [0x09, 0x04, 0xD0, 0x00, 0x34]);
        assert_eq!(Collation::from_bytes(&[1, 2]), Collation::default());
    }
}
