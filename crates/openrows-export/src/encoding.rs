// SPDX-License-Identifier: Apache-2.0
//! Text encodings of delimited files: the byte-order mark and transcoding
//! of encoded text before it reaches the file.
use crate::{ExportError, TextEncoding};

/// The byte-order mark written before any text.
pub(crate) fn preamble(encoding: TextEncoding) -> &'static [u8] {
    match encoding {
        TextEncoding::Utf8Bom => b"\xEF\xBB\xBF",
        TextEncoding::Utf16Le => b"\xFF\xFE",
        TextEncoding::Utf8 | TextEncoding::Latin1 => b"",
    }
}

/// `text` in `encoding`; UTF-8 is borrowed, other encodings fill `bytes`.
///
/// # Errors
/// [`ExportError::Unencodable`] for a character Latin-1 cannot represent.
pub(crate) fn encode<'a>(
    text: &'a str,
    encoding: TextEncoding,
    bytes: &'a mut Vec<u8>,
) -> Result<&'a [u8], ExportError> {
    match encoding {
        TextEncoding::Utf8 | TextEncoding::Utf8Bom => return Ok(text.as_bytes()),
        TextEncoding::Utf16Le => {
            bytes.clear();
            bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
        }
        TextEncoding::Latin1 => {
            bytes.clear();
            for character in text.chars() {
                let byte = u8::try_from(u32::from(character))
                    .map_err(|_| ExportError::Unencodable(character))?;
                bytes.push(byte);
            }
        }
    }
    Ok(bytes.as_slice())
}
