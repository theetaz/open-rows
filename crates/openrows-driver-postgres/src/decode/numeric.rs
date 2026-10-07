// SPDX-License-Identifier: Apache-2.0
//! `PostgreSQL` `numeric` binary format: base-10000 digit groups, never via float.
use super::scalar::CellError;
use std::fmt::{self, Write};

const NEGATIVE: u16 = 0x4000;
const NAN: u16 = 0xC000;
const POSITIVE_INFINITY: u16 = 0xD000;
const NEGATIVE_INFINITY: u16 = 0xF000;
const HEADER: usize = 8;

pub(crate) enum Numeric<'a> {
    Finite(Finite<'a>),
    NaN,
    PositiveInfinity,
    NegativeInfinity,
}

pub(crate) struct Finite<'a> {
    negative: bool,
    /// Base-10000 exponent of the first digit group.
    weight: i32,
    /// Digits after the decimal point in the text form.
    display_scale: u16,
    /// Big-endian `i16` digit groups, each validated to 0..=9999.
    digits: &'a [u8],
}

impl Finite<'_> {
    fn count(&self) -> i32 {
        // Bounded by the wire's i16 digit count.
        i32::try_from(self.digits.len() / 2).unwrap_or(i32::MAX)
    }

    /// Digit group `index` (0 = most significant); zero outside the stored range.
    fn digit(&self, index: i32) -> u16 {
        usize::try_from(index)
            .ok()
            .and_then(|i| self.digits.get(2 * i..2 * i + 2))
            .map_or(0, |pair| u16::from_be_bytes([pair[0], pair[1]]))
    }
}

pub(crate) fn parse(bytes: &[u8]) -> Result<Numeric<'_>, CellError> {
    let header = bytes.get(..HEADER).ok_or(CellError::Truncated)?;
    let field = |at: usize| u16::from_be_bytes([header[at], header[at + 1]]);
    let count = usize::from(field(0));
    let digits = &bytes[HEADER..];
    if count > i16::MAX as usize || digits.len() != 2 * count {
        return Err(CellError::Numeric);
    }
    if digits
        .chunks_exact(2)
        .any(|pair| u16::from_be_bytes([pair[0], pair[1]]) > 9_999)
    {
        return Err(CellError::Numeric);
    }
    match field(4) {
        NAN => Ok(Numeric::NaN),
        POSITIVE_INFINITY => Ok(Numeric::PositiveInfinity),
        NEGATIVE_INFINITY => Ok(Numeric::NegativeInfinity),
        sign @ (0 | NEGATIVE) => Ok(Numeric::Finite(Finite {
            negative: sign == NEGATIVE,
            weight: i32::from(field(2).cast_signed()),
            display_scale: field(6),
            digits,
        })),
        _ => Err(CellError::Numeric),
    }
}

fn pow10(exponent: i32) -> Result<i128, CellError> {
    u32::try_from(exponent)
        .ok()
        .and_then(|e| 10_i128.checked_pow(e))
        .ok_or(CellError::Unrepresentable("numeric"))
}

/// Unscaled `Decimal128` value at `scale`; rejects digits beyond the scale.
pub(crate) fn to_decimal(value: &Finite<'_>, scale: i8) -> Result<i128, CellError> {
    let count = value.count();
    let overflow = || CellError::Unrepresentable("numeric");
    if count == 0 {
        return Ok(0);
    }
    // Exponent applied after Horner evaluation; a negative value trims the last group.
    let exponent = 4 * (value.weight - count + 1) + i32::from(scale);
    let last_shift = 4 + exponent.min(0);
    if last_shift < 1 {
        return Err(CellError::Unrepresentable("numeric scale"));
    }
    let mut acc: i128 = 0;
    for index in 0..count {
        let mut digit = i128::from(value.digit(index));
        let mut shift = 4;
        if index + 1 == count && last_shift < 4 {
            let divisor = pow10(4 - last_shift)?;
            if digit % divisor != 0 {
                return Err(CellError::Unrepresentable("numeric scale"));
            }
            (digit, shift) = (digit / divisor, last_shift);
        }
        acc = acc
            .checked_mul(pow10(shift)?)
            .and_then(|a| a.checked_add(digit))
            .ok_or_else(overflow)?;
    }
    let unscaled = acc
        .checked_mul(pow10(exponent.max(0))?)
        .ok_or_else(overflow)?;
    Ok(if value.negative { -unscaled } else { unscaled })
}

/// `PostgreSQL`'s canonical text form, written without intermediate allocation.
pub(crate) fn write_text(value: &Numeric<'_>, out: &mut impl Write) -> fmt::Result {
    match value {
        Numeric::NaN => out.write_str("NaN"),
        Numeric::PositiveInfinity => out.write_str("Infinity"),
        Numeric::NegativeInfinity => out.write_str("-Infinity"),
        Numeric::Finite(finite) => write_finite(finite, out),
    }
}

fn write_finite(value: &Finite<'_>, out: &mut impl Write) -> fmt::Result {
    if value.negative && value.count() > 0 {
        out.write_char('-')?;
    }
    if value.weight < 0 {
        out.write_char('0')?;
    } else {
        write!(out, "{}", value.digit(0))?;
        for index in 1..=value.weight {
            write!(out, "{:04}", value.digit(index))?;
        }
    }
    if value.display_scale > 0 {
        out.write_char('.')?;
    }
    let mut remaining = u32::from(value.display_scale);
    let mut index = value.weight + 1;
    while remaining > 0 {
        let group = u32::from(value.digit(index));
        for place in (4 - remaining.min(4)..4).rev() {
            let digit = group / 10_u32.pow(place) % 10;
            out.write_char(char::from_digit(digit, 10).unwrap_or('0'))?;
        }
        remaining = remaining.saturating_sub(4);
        index += 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wire(weight: i16, sign: u16, scale: u16, digits: &[u16]) -> Vec<u8> {
        let count = u16::try_from(digits.len()).unwrap_or(u16::MAX);
        let mut bytes = Vec::new();
        for field in [count, weight.cast_unsigned(), sign, scale] {
            bytes.extend_from_slice(&field.to_be_bytes());
        }
        for digit in digits {
            bytes.extend_from_slice(&digit.to_be_bytes());
        }
        bytes
    }

    fn text(bytes: &[u8]) -> String {
        let mut out = String::new();
        let value = parse(bytes).expect("valid numeric");
        write_text(&value, &mut out).expect("string write");
        out
    }

    fn decimal(bytes: &[u8], scale: i8) -> Result<i128, CellError> {
        match parse(bytes)? {
            Numeric::Finite(finite) => to_decimal(&finite, scale),
            _ => Err(CellError::Numeric),
        }
    }

    #[test]
    fn text_matches_postgres_output() {
        assert_eq!(text(&wire(0, 0, 2, &[12, 3400])), "12.34");
        assert_eq!(text(&wire(-1, NEGATIVE, 4, &[5])), "-0.0005");
        assert_eq!(text(&wire(1, 0, 0, &[1])), "10000");
        assert_eq!(text(&wire(0, 0, 3, &[])), "0.000");
        assert_eq!(text(&wire(2, 0, 1, &[1, 0, 2, 5000])), "100000002.5");
        assert_eq!(text(&wire(0, NAN, 0, &[])), "NaN");
        assert_eq!(text(&wire(0, NEGATIVE_INFINITY, 0, &[])), "-Infinity");
    }

    #[test]
    fn decimal_scales_exactly() {
        assert_eq!(decimal(&wire(0, 0, 2, &[12, 3400]), 2), Ok(1234));
        assert_eq!(decimal(&wire(0, 0, 2, &[12, 3400]), 4), Ok(123_400));
        assert_eq!(decimal(&wire(-1, NEGATIVE, 4, &[5]), 4), Ok(-5));
        assert_eq!(decimal(&wire(1, 0, 0, &[1]), 0), Ok(10_000));
        assert_eq!(decimal(&wire(0, 0, 0, &[]), 10), Ok(0));
    }

    #[test]
    fn decimal_handles_38_digits_without_intermediate_overflow() {
        // 0.12345678901234567890123456789012345678 in numeric(38, 38)
        let groups = [1234, 5678, 9012, 3456, 7890, 1234, 5678, 9012, 3456, 7800];
        let expected = 12_345_678_901_234_567_890_123_456_789_012_345_678;
        assert_eq!(decimal(&wire(-1, 0, 38, &groups), 38), Ok(expected));
        let max = [9999; 10];
        assert_eq!(
            decimal(&wire(9, 0, 0, &max), 0),
            Err(CellError::Unrepresentable("numeric"))
        );
    }

    #[test]
    fn malformed_input_is_rejected() {
        assert!(matches!(parse(&[0, 1]), Err(CellError::Truncated)));
        assert!(matches!(
            parse(&wire(0, 0, 0, &[10_000])),
            Err(CellError::Numeric)
        ));
        assert!(matches!(
            parse(&wire(0, 0x1234, 0, &[])),
            Err(CellError::Numeric)
        ));
        let mut short = wire(0, 0, 0, &[1, 2]);
        short.pop();
        assert!(matches!(parse(&short), Err(CellError::Numeric)));
        assert_eq!(
            decimal(&wire(-1, 0, 4, &[1, 5]), 4),
            Err(CellError::Unrepresentable("numeric scale"))
        );
    }
}
