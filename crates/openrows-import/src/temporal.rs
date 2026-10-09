// SPDX-License-Identifier: Apache-2.0
//! ISO 8601 date and timestamp recognition shared by inference and value
//! checks: `YYYY-MM-DD`, then optionally `T` or a space, `HH:MM[:SS[.f]]`
//! and a zone (`Z`, `±HH`, `±HH:MM`, `±HHMM`).

/// What a temporal text holds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Temporal {
    Date,
    Timestamp,
    TimestampTz,
}

fn digits(text: &str, count: usize) -> Option<u32> {
    (text.len() == count && text.bytes().all(|b| b.is_ascii_digit()))
        .then(|| text.parse().ok())
        .flatten()
}

fn days_in(year: u32, month: u32) -> u32 {
    match month {
        2 if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) => {
            29
        }
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

fn valid_date(text: &str) -> bool {
    let mut parts = text.split('-');
    let (Some(year), Some(month), Some(day), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    let (Some(year), Some(month), Some(day)) = (digits(year, 4), digits(month, 2), digits(day, 2))
    else {
        return false;
    };
    (1..=12).contains(&month) && day >= 1 && day <= days_in(year, month)
}

fn valid_time(text: &str) -> bool {
    let (clock, fraction) = text.split_once('.').unwrap_or((text, ""));
    if !fraction.is_empty() && (fraction.len() > 9 || !fraction.bytes().all(|b| b.is_ascii_digit()))
    {
        return false;
    }
    if text.contains('.') && fraction.is_empty() {
        return false;
    }
    let mut parts = clock.split(':');
    let hour = parts.next().and_then(|part| digits(part, 2));
    let minute = parts.next().and_then(|part| digits(part, 2));
    let second = match parts.next() {
        Some(part) => digits(part, 2),
        None if fraction.is_empty() => Some(0),
        None => None,
    };
    matches!((hour, minute, second, parts.next()),
        (Some(h), Some(m), Some(s), None) if h < 24 && m < 60 && s < 61)
}

fn valid_zone(zone: &str) -> bool {
    if zone == "Z" {
        return true;
    }
    let offset = &zone[1..];
    let (hours, minutes) = match offset.len() {
        2 => (offset, "00"),
        4 => offset.split_at(2),
        5 if offset.as_bytes()[2] == b':' => (&offset[..2], &offset[3..]),
        _ => return false,
    };
    matches!((digits(hours, 2), digits(minutes, 2)), (Some(h), Some(m)) if h < 24 && m < 60)
}

/// Split a time-of-day from its zone suffix, if any.
fn split_zone(text: &str) -> (&str, Option<&str>) {
    if let Some(time) = text.strip_suffix('Z') {
        return (time, Some("Z"));
    }
    match text.rfind(['+', '-']) {
        Some(index) => (&text[..index], Some(&text[index..])),
        None => (text, None),
    }
}

/// The temporal kind of `text`, or `None` when it is not one.
pub(crate) fn temporal(text: &str) -> Option<Temporal> {
    if text.len() < 10 || !text.is_char_boundary(10) {
        return None;
    }
    let (date, rest) = text.split_at(10);
    if !valid_date(date) {
        return None;
    }
    if rest.is_empty() {
        return Some(Temporal::Date);
    }
    let time = rest.strip_prefix(['T', ' '])?;
    let (clock, zone) = split_zone(time);
    if !valid_time(clock) {
        return None;
    }
    match zone {
        None => Some(Temporal::Timestamp),
        Some(zone) if valid_zone(zone) => Some(Temporal::TimestampTz),
        Some(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_and_timestamps_are_recognised_strictly() {
        assert_eq!(temporal("2024-02-29"), Some(Temporal::Date));
        assert_eq!(temporal("2023-02-29"), None);
        assert_eq!(temporal("2024-13-01"), None);
        assert_eq!(temporal("2024-1-01"), None);
        assert_eq!(temporal("2024-01-02 03:04"), Some(Temporal::Timestamp));
        assert_eq!(
            temporal("2024-01-02T03:04:05.123456"),
            Some(Temporal::Timestamp)
        );
        assert_eq!(
            temporal("2024-01-02T03:04:05Z"),
            Some(Temporal::TimestampTz)
        );
        assert_eq!(
            temporal("2024-01-02 03:04:05+05:30"),
            Some(Temporal::TimestampTz)
        );
        assert_eq!(
            temporal("2024-01-02 03:04:05-0800"),
            Some(Temporal::TimestampTz)
        );
        assert_eq!(temporal("2024-01-02 25:00"), None);
        assert_eq!(temporal("2024-01-02 03:04:05."), None);
        assert_eq!(temporal("2024-01-02x03:04"), None);
        assert_eq!(temporal("2024-01-02 03:04+5"), None);
        assert_eq!(temporal("ünïcode-text"), None);
        assert_eq!(temporal("hello"), None);
    }
}
