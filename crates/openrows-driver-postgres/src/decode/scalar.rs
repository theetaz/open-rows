// SPDX-License-Identifier: Apache-2.0
//! Fixed-width `PostgreSQL` binary values (big-endian) and epoch conversion.

/// Why one cell could not be decoded.
#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
pub(crate) enum CellError {
    #[error("expected {expected} bytes, got {actual}")]
    Length { expected: usize, actual: usize },
    #[error("truncated value")]
    Truncated,
    #[error("invalid UTF-8 text")]
    InvalidUtf8,
    #[error("unsupported jsonb version {0}")]
    JsonbVersion(u8),
    #[error("malformed numeric")]
    Numeric,
    /// docs/04 §5 wants tagged text here; that needs a per-value fallback (M1-11).
    #[error("{0} cannot be represented in the column's Arrow type")]
    Unrepresentable(&'static str),
}

/// Microseconds between 1970-01-01 and `PostgreSQL`'s 2000-01-01 epoch.
const PG_EPOCH_MICROS: i64 = 946_684_800_000_000;
/// Days between 1970-01-01 and 2000-01-01.
const PG_EPOCH_DAYS: i32 = 10_957;

pub(crate) fn array<const N: usize>(bytes: &[u8]) -> Result<[u8; N], CellError> {
    bytes.try_into().map_err(|_| CellError::Length {
        expected: N,
        actual: bytes.len(),
    })
}

pub(crate) fn timestamp_micros(bytes: &[u8]) -> Result<i64, CellError> {
    let micros = i64::from_be_bytes(array(bytes)?);
    if micros == i64::MAX || micros == i64::MIN {
        return Err(CellError::Unrepresentable("infinite timestamp"));
    }
    micros
        .checked_add(PG_EPOCH_MICROS)
        .ok_or(CellError::Unrepresentable("timestamp"))
}

pub(crate) fn date_days(bytes: &[u8]) -> Result<i32, CellError> {
    let days = i32::from_be_bytes(array(bytes)?);
    if days == i32::MAX || days == i32::MIN {
        return Err(CellError::Unrepresentable("infinite date"));
    }
    days.checked_add(PG_EPOCH_DAYS)
        .ok_or(CellError::Unrepresentable("date"))
}

pub(crate) fn utf8(bytes: &[u8]) -> Result<&str, CellError> {
    std::str::from_utf8(bytes).map_err(|_| CellError::InvalidUtf8)
}

/// jsonb's binary send format is a version byte (1) followed by JSON text.
pub(crate) fn jsonb(bytes: &[u8]) -> Result<&str, CellError> {
    match bytes.split_first() {
        Some((1, text)) => utf8(text),
        Some((&version, _)) => Err(CellError::JsonbVersion(version)),
        None => Err(CellError::Truncated),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epochs_shift_from_2000_to_1970() {
        assert_eq!(timestamp_micros(&0_i64.to_be_bytes()), Ok(PG_EPOCH_MICROS));
        assert_eq!(date_days(&(-PG_EPOCH_DAYS).to_be_bytes()), Ok(0));
    }

    #[test]
    fn infinities_and_wrong_lengths_are_typed_errors() {
        assert!(matches!(
            timestamp_micros(&i64::MAX.to_be_bytes()),
            Err(CellError::Unrepresentable(_))
        ));
        assert!(matches!(
            date_days(&i32::MIN.to_be_bytes()),
            Err(CellError::Unrepresentable(_))
        ));
        assert_eq!(
            array::<4>(&[1, 2]),
            Err(CellError::Length {
                expected: 4,
                actual: 2
            })
        );
    }

    #[test]
    fn jsonb_strips_the_version_byte() {
        assert_eq!(jsonb(b"\x01{\"a\": 1}"), Ok("{\"a\": 1}"));
        assert_eq!(jsonb(b"\x02{}"), Err(CellError::JsonbVersion(2)));
        assert_eq!(jsonb(b""), Err(CellError::Truncated));
        assert_eq!(utf8(b"\xff"), Err(CellError::InvalidUtf8));
    }
}
