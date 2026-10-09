// SPDX-License-Identifier: Apache-2.0
//! Type inference over a sample of rows (FR-IO-06): each value is recognised
//! strictly (no leading zeros, only `true`/`false`, ISO dates), and the
//! categories seen in a column merge to the narrowest one that holds all.
use crate::temporal::{Temporal, temporal};
use crate::types::{TypeCategory, is_decimal, is_integer};

/// The category a single non-empty value suggests.
fn recognise(value: &str) -> TypeCategory {
    let digits = value.strip_prefix(['+', '-']).unwrap_or(value);
    if is_integer(value) {
        // Leading zeros (codes, ZIP numbers) and out-of-range numbers stay exact.
        let leading_zero = digits.len() > 1 && digits.starts_with('0');
        return match (leading_zero, value.parse::<i64>().is_ok()) {
            (true, _) => TypeCategory::Text,
            (false, true) => TypeCategory::Integer,
            (false, false) => TypeCategory::Decimal,
        };
    }
    if is_decimal(value) {
        let leading_zero = digits.len() > 1 && digits.starts_with('0') && !digits.starts_with("0.");
        return if leading_zero {
            TypeCategory::Text
        } else {
            TypeCategory::Decimal
        };
    }
    if value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("false") {
        return TypeCategory::Boolean;
    }
    match temporal(value) {
        Some(Temporal::Date) => return TypeCategory::Date,
        Some(Temporal::Timestamp) => return TypeCategory::Timestamp,
        Some(Temporal::TimestampTz) => return TypeCategory::TimestampTz,
        None => {}
    }
    let structured = value.starts_with('{') || value.starts_with('[');
    if structured && TypeCategory::Json.accepts(value) {
        return TypeCategory::Json;
    }
    TypeCategory::Text
}

/// The narrowest category holding values of both.
fn merge(left: TypeCategory, right: TypeCategory) -> TypeCategory {
    use TypeCategory::{Date, Decimal, Integer, Text, Timestamp};
    match (left, right) {
        _ if left == right => left,
        (Integer, Decimal) | (Decimal, Integer) => Decimal,
        (Date, Timestamp) | (Timestamp, Date) => Timestamp,
        _ => Text,
    }
}

/// What the sample showed about one column.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ColumnStats {
    /// The merged category; `None` while every value was NULL.
    seen: Option<TypeCategory>,
    /// A NULL (missing or empty) value was seen.
    pub nullable: bool,
    /// Longest value, in characters.
    pub longest: usize,
}

impl ColumnStats {
    /// Account for one value of the column.
    pub fn observe(&mut self, value: Option<&str>) {
        let Some(value) = value else {
            self.nullable = true;
            return;
        };
        self.longest = self.longest.max(value.chars().count());
        let category = recognise(value.trim());
        self.seen = Some(self.seen.map_or(category, |seen| merge(seen, category)));
    }

    /// The inferred category; text when only NULLs were seen.
    #[must_use]
    pub fn category(&self) -> TypeCategory {
        self.seen.unwrap_or(TypeCategory::Text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use TypeCategory::*;

    fn infer(values: &[Option<&str>]) -> (TypeCategory, bool) {
        let mut stats = ColumnStats::default();
        for value in values {
            stats.observe(*value);
        }
        (stats.category(), stats.nullable)
    }

    #[test]
    fn single_values_are_recognised_strictly() {
        for (value, category) in [
            ("42", Integer),
            ("-7", Integer),
            ("007", Text),
            ("0", Integer),
            ("99999999999999999999", Decimal),
            ("3.14", Decimal),
            ("0.5", Decimal),
            ("1e-3", Decimal),
            ("01.5", Text),
            ("TRUE", Boolean),
            ("yes", Text),
            ("2024-01-02", Date),
            ("2024-01-02 03:04:05", Timestamp),
            ("2024-01-02T03:04:05Z", TimestampTz),
            ("{\"a\": 1}", Json),
            ("[1, 2]", Json),
            ("{not json}", Text),
            ("hello", Text),
        ] {
            assert_eq!(recognise(value), category, "{value}");
        }
    }

    #[test]
    fn columns_merge_to_the_narrowest_category() {
        assert_eq!(infer(&[Some("1"), Some("2.5")]), (Decimal, false));
        assert_eq!(infer(&[Some("1"), None, Some("3")]), (Integer, true));
        assert_eq!(
            infer(&[Some("2024-01-02"), Some("2024-01-02 10:00")]),
            (Timestamp, false)
        );
        assert_eq!(infer(&[Some("1"), Some("x")]), (Text, false));
        assert_eq!(infer(&[Some("true"), Some("1")]), (Text, false));
        assert_eq!(infer(&[None, None]), (Text, true));
        assert_eq!(infer(&[]), (Text, false));
    }

    #[test]
    fn the_longest_value_is_counted_in_characters() {
        let mut stats = ColumnStats::default();
        stats.observe(Some("ünï"));
        stats.observe(Some("ab"));
        assert_eq!(stats.longest, 3);
    }
}
