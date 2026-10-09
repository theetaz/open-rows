// SPDX-License-Identifier: Apache-2.0
//! Typed values of an open batch printed back as the server's text when
//! their column falls back to tagged text.
use std::fmt;

use arrow_array::builder::StringBuilder;
use arrow_array::cast::AsArray;
use arrow_array::types::{
    Date32Type, Decimal128Type, Time64MicrosecondType, TimestampMicrosecondType,
};
use arrow_array::{Array, ArrowPrimitiveType};

use super::{Kind, decimal, temporal};

/// Append every value of `values` (a finished column of `kind`) as text.
/// Only temporal and decimal kinds fall back; others never reach here.
pub(super) fn append_array(kind: Kind, values: &dyn Array, out: &mut StringBuilder) {
    match kind {
        Kind::Date => each::<Date32Type>(values, out, temporal::write_date),
        Kind::DateTime { fsp } => each::<TimestampMicrosecondType>(values, out, |micros, out| {
            temporal::write_datetime(micros, fsp, out)
        }),
        Kind::Time { fsp } => each::<Time64MicrosecondType>(values, out, |micros, out| {
            temporal::write_time(micros, fsp, out)
        }),
        Kind::Decimal { scale, .. } => each::<Decimal128Type>(values, out, |unscaled, out| {
            decimal::write(unscaled, scale, out)
        }),
        _ => {}
    }
}

fn each<T: ArrowPrimitiveType>(
    values: &dyn Array,
    out: &mut StringBuilder,
    mut write: impl FnMut(T::Native, &mut String) -> fmt::Result,
) {
    let Some(values) = values.as_primitive_opt::<T>() else {
        return;
    };
    let mut text = String::new();
    for value in values {
        match value {
            Some(value) => {
                text.clear();
                // Writing to a `String` cannot fail.
                let _ = write(value, &mut text);
                out.append_value(&text);
            }
            None => out.append_null(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::{Date32Array, Decimal128Array, Int64Array, Time64MicrosecondArray};

    fn texts(kind: Kind, values: &dyn Array) -> Vec<Option<String>> {
        let mut out = StringBuilder::new();
        append_array(kind, values, &mut out);
        out.finish()
            .iter()
            .map(|value| value.map(str::to_owned))
            .collect()
    }

    #[test]
    fn typed_values_print_as_server_text_and_nulls_stay_null() {
        let dates = Date32Array::from(vec![Some(0), None]);
        assert_eq!(
            texts(Kind::Date, &dates),
            [Some("1970-01-01".to_owned()), None]
        );
        let times = Time64MicrosecondArray::from(vec![3_600_000_000]);
        assert_eq!(
            texts(Kind::Time { fsp: Some(1) }, &times),
            [Some("01:00:00.0".to_owned())]
        );
        let decimals = Decimal128Array::from(vec![-1_250]);
        let kind = Kind::Decimal {
            precision: 10,
            scale: 2,
        };
        assert_eq!(texts(kind, &decimals), [Some("-12.50".to_owned())]);
    }

    #[test]
    fn a_mismatched_array_adds_nothing() {
        let ints = Int64Array::from(vec![1]);
        assert!(texts(Kind::Date, &ints).is_empty());
    }
}
