// SPDX-License-Identifier: Apache-2.0
use crate::text::{Temporal, is_number};
use arrow_schema::TimeUnit;

#[test]
fn only_json_numbers_count_as_numbers() {
    for good in ["0", "-0", "12", "-12.50", "1e5", "1.5E-3", "2e+10", "0.001"] {
        assert!(is_number(good), "{good}");
    }
    for bad in [
        "", "-", "01", ".5", "5.", "1e", "1e+", "NaN", "Infinity", "+1", "1_000", " 1", "1 ",
        "0x1f",
    ] {
        assert!(!is_number(bad), "{bad}");
    }
}

#[test]
fn temporals_render_in_iso_form_for_every_unit() {
    let at = |value, unit, utc| Temporal::Timestamp { value, unit, utc }.to_string();
    assert_eq!(at(0, TimeUnit::Second, false), "1970-01-01 00:00:00");
    assert_eq!(
        at(1_500, TimeUnit::Millisecond, true),
        "1970-01-01 00:00:01.500+00:00"
    );
    assert_eq!(
        at(-1, TimeUnit::Microsecond, false),
        "1969-12-31 23:59:59.999999"
    );
    assert_eq!(
        at(1, TimeUnit::Nanosecond, false),
        "1970-01-01 00:00:00.000000001"
    );
    assert_eq!(Temporal::Date(0).to_string(), "1970-01-01");
    assert_eq!(Temporal::DateMillis(86_400_000).to_string(), "1970-01-02");
    assert_eq!(Temporal::Date(i32::MAX).to_string(), i32::MAX.to_string());
}

#[test]
fn times_and_intervals_render_like_postgres() {
    use arrow_array::types::IntervalMonthDayNano;
    let time = |value, unit| Temporal::Time { value, unit }.to_string();
    assert_eq!(time(3_723_500_000, TimeUnit::Microsecond), "01:02:03.5");
    assert_eq!(time(61, TimeUnit::Second), "00:01:01");
    assert_eq!(time(1_000, TimeUnit::Nanosecond), "00:00:00.000001");
    let interval = Temporal::Interval(IntervalMonthDayNano::new(14, -1, 3_600_000_000_000));
    assert_eq!(interval.to_string(), "1 year 2 mons -1 days +01:00:00");
}

#[test]
fn lists_render_as_postgres_array_text() -> Result<(), Box<dyn std::error::Error>> {
    use crate::cell::Column;
    use crate::text::push_plain;
    use arrow_array::builder::{ListBuilder, StringBuilder};
    use arrow_array::types::Int32Type;
    use arrow_array::{Array, ListArray};
    use arrow_schema::Field;
    let ints = ListArray::from_iter_primitive::<Int32Type, _, _>(vec![
        Some(vec![Some(1), None, Some(3)]),
        Some(vec![]),
    ]);
    let field = Field::new("l", ints.data_type().clone(), true);
    let column = Column::new(&field, &ints)?;
    let mut out = String::new();
    push_plain(&column.cell(0), &mut out);
    out.push('|');
    push_plain(&column.cell(1), &mut out);
    assert_eq!(out, "{1,NULL,3}|{}");
    let mut builder = ListBuilder::new(StringBuilder::new());
    for word in ["a b", "", "x"] {
        builder.values().append_value(word);
    }
    builder.append(true);
    let words = builder.finish();
    let field = Field::new("w", words.data_type().clone(), true);
    out.clear();
    push_plain(&Column::new(&field, &words)?.cell(0), &mut out);
    assert_eq!(out, r#"{"a b","",x}"#);
    Ok(())
}
