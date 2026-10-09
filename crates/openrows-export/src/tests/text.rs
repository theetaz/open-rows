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
