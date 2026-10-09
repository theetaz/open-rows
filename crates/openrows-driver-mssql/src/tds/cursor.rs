// SPDX-License-Identifier: Apache-2.0
//! A little-endian reader over a token buffer that reports running out of
//! bytes as [`Short::Incomplete`], so a token split across packets is parsed
//! again once more bytes arrived.

use openrows_driver_api::DriverError;

/// Why a parse stopped early.
#[derive(Debug)]
pub(crate) enum Short {
    /// The buffer ends inside the token; read more and retry.
    Incomplete,
    /// The bytes are not valid TDS.
    Invalid(DriverError),
}

impl From<DriverError> for Short {
    fn from(error: DriverError) -> Self {
        Self::Invalid(error)
    }
}

/// A parse result that may need more bytes.
pub(crate) type Parsed<T> = Result<T, Short>;

/// A protocol error naming what was malformed.
pub(crate) fn invalid<T>(what: impl Into<String>) -> Parsed<T> {
    Err(Short::Invalid(DriverError::Protocol(what.into())))
}

/// Reads values from `bytes` starting at `pos`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    /// Bytes consumed so far.
    pub(crate) fn position(&self) -> usize {
        self.pos
    }

    /// The next `len` bytes.
    pub(crate) fn take(&mut self, len: usize) -> Parsed<&'a [u8]> {
        let end = self.pos.checked_add(len).ok_or(Short::Incomplete)?;
        let slice = self.bytes.get(self.pos..end).ok_or(Short::Incomplete)?;
        self.pos = end;
        Ok(slice)
    }

    pub(crate) fn skip(&mut self, len: usize) -> Parsed<()> {
        self.take(len).map(drop)
    }

    fn array<const N: usize>(&mut self) -> Parsed<[u8; N]> {
        let slice = self.take(N)?;
        let mut out = [0_u8; N];
        out.copy_from_slice(slice);
        Ok(out)
    }

    pub(crate) fn u8(&mut self) -> Parsed<u8> {
        Ok(self.array::<1>()?[0])
    }

    pub(crate) fn u16(&mut self) -> Parsed<u16> {
        self.array().map(u16::from_le_bytes)
    }

    pub(crate) fn u32(&mut self) -> Parsed<u32> {
        self.array().map(u32::from_le_bytes)
    }

    pub(crate) fn i32(&mut self) -> Parsed<i32> {
        self.array().map(i32::from_le_bytes)
    }

    pub(crate) fn u64(&mut self) -> Parsed<u64> {
        self.array().map(u64::from_le_bytes)
    }

    /// UTF-16LE text of `chars` code units.
    pub(crate) fn utf16(&mut self, chars: usize) -> Parsed<String> {
        let bytes = self.take(chars.checked_mul(2).ok_or(Short::Incomplete)?)?;
        Ok(utf16_text(bytes))
    }

    /// `B_VARCHAR`: a one-byte character count, then UTF-16LE.
    pub(crate) fn b_varchar(&mut self) -> Parsed<String> {
        let chars = self.u8()?;
        self.utf16(usize::from(chars))
    }

    /// `US_VARCHAR`: a two-byte character count, then UTF-16LE.
    pub(crate) fn us_varchar(&mut self) -> Parsed<String> {
        let chars = self.u16()?;
        self.utf16(usize::from(chars))
    }

    /// `B_VARBYTE`: a one-byte length, then bytes.
    pub(crate) fn b_varbyte(&mut self) -> Parsed<&'a [u8]> {
        let len = self.u8()?;
        self.take(usize::from(len))
    }
}

/// UTF-16LE bytes as text; unpaired surrogates become U+FFFD.
pub(crate) fn utf16_text(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    String::from_utf16_lossy(&units)
}

/// `text` as UTF-16LE bytes.
pub(crate) fn utf16_bytes(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_read_little_endian_and_stop_at_the_end() {
        let mut cursor = Cursor::new(&[1, 0x34, 0x12, 0x78, 0x56, 0x34, 0x12, 9]);
        assert!(matches!(cursor.u8(), Ok(1)));
        assert!(matches!(cursor.u16(), Ok(0x1234)));
        assert!(matches!(cursor.u32(), Ok(0x1234_5678)));
        assert!(matches!(cursor.u16(), Err(Short::Incomplete)));
        assert_eq!(cursor.position(), 7);
    }

    #[test]
    fn utf16_text_round_trips_and_tolerates_lone_surrogates() {
        let bytes = utf16_bytes("h\u{e9}llo \u{1f600}");
        let mut cursor = Cursor::new(&bytes);
        assert_eq!(
            cursor.utf16(bytes.len() / 2).ok().as_deref(),
            Some("h\u{e9}llo \u{1f600}")
        );
        assert_eq!(utf16_text(&[0x00, 0xd8]), "\u{fffd}");
        let mut named = vec![2_u8];
        named.extend(utf16_bytes("ab"));
        assert_eq!(Cursor::new(&named).b_varchar().ok().as_deref(), Some("ab"));
    }
}
