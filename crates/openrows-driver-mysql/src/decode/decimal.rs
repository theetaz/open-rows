// SPDX-License-Identifier: Apache-2.0
//! `DECIMAL(p, s)` text to an unscaled `Decimal128` value and back, digit by
//! digit, never through a float. A value with more digits than the column's
//! precision or scale allows is [`CellError::Unrepresentable`].
use std::fmt::Write;

use super::CellError;

const UNREPRESENTABLE: CellError = CellError::Unrepresentable("decimal outside its precision");

/// The unscaled value of `text` at `scale`, with at most `precision` digits.
pub(super) fn parse(text: &[u8], precision: u8, scale: i8) -> Result<i128, CellError> {
    let scale = usize::try_from(scale).map_err(|_| UNREPRESENTABLE)?;
    let (negative, digits) = match text.split_first() {
        Some((b'-', rest)) => (true, rest),
        Some((b'+', rest)) => (false, rest),
        _ => (false, text),
    };
    let (whole, fraction) = match digits.iter().position(|&byte| byte == b'.') {
        Some(point) => (&digits[..point], &digits[point + 1..]),
        None => (digits, &[][..]),
    };
    let all_digits = |part: &[u8]| part.iter().all(u8::is_ascii_digit);
    if whole.is_empty() && fraction.is_empty() || !all_digits(whole) || !all_digits(fraction) {
        return Err(UNREPRESENTABLE);
    }
    let (kept, dropped) = fraction.split_at(fraction.len().min(scale));
    if dropped.iter().any(|&digit| digit != b'0') {
        return Err(UNREPRESENTABLE);
    }
    let whole = trim_leading_zeros(whole);
    if whole.len() + scale > usize::from(precision) {
        return Err(UNREPRESENTABLE);
    }
    let padding = std::iter::repeat_n(&b'0', scale - kept.len());
    let magnitude = whole
        .iter()
        .chain(kept)
        .chain(padding)
        .try_fold(0_i128, |value, digit| {
            value.checked_mul(10)?.checked_add(i128::from(digit - b'0'))
        })
        .ok_or(UNREPRESENTABLE)?;
    Ok(if negative { -magnitude } else { magnitude })
}

fn trim_leading_zeros(digits: &[u8]) -> &[u8] {
    let zeros = digits.iter().take_while(|&&digit| digit == b'0').count();
    &digits[zeros..]
}

/// The server's text for an unscaled value: every scale digit is printed.
pub(super) fn write(unscaled: i128, scale: i8, out: &mut impl Write) -> std::fmt::Result {
    let scale = u32::try_from(scale).unwrap_or(0).min(38);
    let divisor = 10_u128.pow(scale);
    let magnitude = unscaled.unsigned_abs();
    let sign = if unscaled < 0 { "-" } else { "" };
    write!(out, "{sign}{}", magnitude / divisor)?;
    if scale > 0 {
        let width = usize::try_from(scale).unwrap_or(0);
        write!(out, ".{:0width$}", magnitude % divisor)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimals_parse_exactly_at_the_column_scale() {
        assert_eq!(parse(b"12.50", 10, 2), Ok(1_250));
        assert_eq!(parse(b"-0.05", 10, 2), Ok(-5));
        assert_eq!(parse(b"7", 10, 2), Ok(700));
        assert_eq!(parse(b"7.", 10, 2), Ok(700));
        assert_eq!(parse(b".5", 3, 1), Ok(5));
        assert_eq!(parse(b"001.10", 3, 2), Ok(110));
        assert_eq!(parse(b"1.230", 5, 2), Ok(123), "zero digits past the scale");
        let max = b"99999999999999999999999999999999999999";
        assert_eq!(parse(max, 38, 0), Ok(10_i128.pow(38) - 1));
    }

    #[test]
    fn digits_outside_precision_or_scale_are_unrepresentable() {
        for (text, precision, scale) in [
            (&b"1.234"[..], 10, 2),
            (b"1000", 3, 0),
            (b"10.00", 3, 2),
            (b"", 3, 0),
            (b"-", 3, 0),
            (b"1e5", 10, 0),
            (b"1.2.3", 10, 2),
            (b" 1", 10, 0),
        ] {
            assert_eq!(
                parse(text, precision, scale),
                Err(UNREPRESENTABLE),
                "{text:?}"
            );
        }
    }

    #[test]
    fn unscaled_values_print_every_scale_digit() {
        let text = |value: i128, scale: i8| {
            let mut out = String::new();
            assert!(write(value, scale, &mut out).is_ok());
            out
        };
        assert_eq!(text(1_250, 2), "12.50");
        assert_eq!(text(-5, 2), "-0.05");
        assert_eq!(text(42, 0), "42");
        assert_eq!(text(5, 3), "0.005");
    }
}
