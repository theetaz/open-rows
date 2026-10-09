// SPDX-License-Identifier: Apache-2.0
//! Numbers, `bit` and `uniqueidentifier` from their little-endian wire
//! form, and the exact text of a scaled decimal.

use std::fmt::Write as _;

use super::CellError;

fn malformed(what: &str, bytes: &[u8]) -> CellError {
    CellError::Malformed(format!("{} bytes are no {what} value", bytes.len()))
}

/// `N` bytes as a fixed array.
fn exact<const N: usize>(bytes: &[u8], what: &str) -> Result<[u8; N], CellError> {
    bytes.try_into().map_err(|_| malformed(what, bytes))
}

pub(crate) fn tinyint(bytes: &[u8]) -> Result<i16, CellError> {
    exact::<1>(bytes, "tinyint").map(|[byte]| i16::from(byte))
}

pub(crate) fn smallint(bytes: &[u8]) -> Result<i16, CellError> {
    exact(bytes, "smallint").map(i16::from_le_bytes)
}

pub(crate) fn int(bytes: &[u8]) -> Result<i32, CellError> {
    exact(bytes, "int").map(i32::from_le_bytes)
}

pub(crate) fn bigint(bytes: &[u8]) -> Result<i64, CellError> {
    exact(bytes, "bigint").map(i64::from_le_bytes)
}

pub(crate) fn bit(bytes: &[u8]) -> Result<bool, CellError> {
    exact::<1>(bytes, "bit").map(|[byte]| byte != 0)
}

pub(crate) fn real(bytes: &[u8]) -> Result<f32, CellError> {
    exact(bytes, "real").map(f32::from_le_bytes)
}

pub(crate) fn float(bytes: &[u8]) -> Result<f64, CellError> {
    exact(bytes, "float").map(f64::from_le_bytes)
}

/// `money` (high 32 bits first) or `smallmoney`, in ten-thousandths.
pub(crate) fn money(bytes: &[u8]) -> Result<i128, CellError> {
    match bytes {
        [a, b, c, d] => Ok(i128::from(i32::from_le_bytes([*a, *b, *c, *d]))),
        [h0, h1, h2, h3, l0, l1, l2, l3] => {
            let high = i64::from(i32::from_le_bytes([*h0, *h1, *h2, *h3]));
            let low = i64::from(u32::from_le_bytes([*l0, *l1, *l2, *l3]));
            Ok(i128::from(high << 32 | low))
        }
        _ => Err(malformed("money", bytes)),
    }
}

/// `decimal`: a sign byte (1 is positive), then the unscaled magnitude.
pub(crate) fn decimal(bytes: &[u8]) -> Result<i128, CellError> {
    let (sign, magnitude) = bytes
        .split_first()
        .filter(|(_, magnitude)| magnitude.len() <= 16)
        .ok_or_else(|| malformed("decimal", bytes))?;
    let value = magnitude
        .iter()
        .rev()
        .fold(0_u128, |value, byte| value << 8 | u128::from(*byte));
    let value = i128::try_from(value).map_err(|_| malformed("decimal", bytes))?;
    Ok(if *sign == 1 { value } else { -value })
}

/// A `uniqueidentifier` in RFC 4122 byte order (its first three groups
/// travel little-endian).
pub(crate) fn guid(bytes: &[u8]) -> Result<[u8; 16], CellError> {
    let mut out: [u8; 16] = exact(bytes, "uniqueidentifier")?;
    out[..4].reverse();
    out[4..6].reverse();
    out[6..8].reverse();
    Ok(out)
}

/// `unscaled / 10^scale` with exactly `scale` fraction digits.
pub(crate) fn write_scaled(unscaled: i128, scale: u8, out: &mut String) {
    let digits = unscaled.unsigned_abs().to_string();
    let scale = usize::from(scale);
    let padded = format!("{digits:0>width$}", width = scale + 1);
    let (whole, fraction) = padded.split_at(padded.len() - scale);
    if unscaled < 0 {
        out.push('-');
    }
    out.push_str(whole);
    if scale > 0 {
        let _ = write!(out, ".{fraction}");
    }
}

/// `XXXXXXXX-XXXX-…` in upper case, as SQL Server prints it.
pub(crate) fn write_guid(rfc: &[u8; 16], out: &mut String) {
    for (index, byte) in rfc.iter().enumerate() {
        if matches!(index, 4 | 6 | 8 | 10) {
            out.push('-');
        }
        let _ = write!(out, "{byte:02X}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scaled(value: i128, scale: u8) -> String {
        let mut out = String::new();
        write_scaled(value, scale, &mut out);
        out
    }

    #[test]
    fn decimals_and_money_keep_every_digit() -> Result<(), CellError> {
        assert_eq!(decimal(&[1, 0xE2, 0x04, 0, 0])?, 1250);
        assert_eq!(decimal(&[0, 5, 0, 0, 0])?, -5);
        assert!(decimal(&[]).is_err());
        assert_eq!(scaled(1250, 2), "12.50");
        assert_eq!(scaled(-5, 3), "-0.005");
        assert_eq!(scaled(42, 0), "42");
        let mut money_bytes = 0_i32.to_le_bytes().to_vec();
        money_bytes.extend(123_456_u32.to_le_bytes());
        assert_eq!(money(&money_bytes)?, 123_456);
        let negative = (-1_i64).to_le_bytes();
        let swapped = [&negative[4..], &negative[..4]].concat();
        assert_eq!(money(&swapped)?, -1);
        assert_eq!(money(&(-50_i32).to_le_bytes())?, -50);
        Ok(())
    }

    #[test]
    fn integers_floats_bits_and_guids_decode() -> Result<(), CellError> {
        assert_eq!(tinyint(&[255])?, 255);
        assert_eq!(smallint(&(-2_i16).to_le_bytes())?, -2);
        assert_eq!(int(&7_i32.to_le_bytes())?, 7);
        assert_eq!(bigint(&i64::MIN.to_le_bytes())?, i64::MIN);
        assert!(bit(&[1])?);
        assert!((float(&1.5_f64.to_le_bytes())? - 1.5).abs() < f64::EPSILON);
        assert!(int(&[1, 2]).is_err());
        let wire = [
            0x33, 0x22, 0x11, 0x00, 0x55, 0x44, 0x77, 0x66, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD,
            0xEE, 0xFF,
        ];
        let rfc = guid(&wire)?;
        let mut text = String::new();
        write_guid(&rfc, &mut text);
        assert_eq!(text, "00112233-4455-6677-8899-AABBCCDDEEFF");
        Ok(())
    }
}
