// SPDX-License-Identifier: Apache-2.0
//! Read-only admission and typed binding of widget SQL: values travel as
//! bound parameters, never as SQL text.
use openrows_sql::select::BoundSql;
use openrows_sql::{Dialect, ParamBinder, ParamSlot, ParamStyle, StatementKind, classify, split};

use super::BoardError;
use super::model::{Board, BoardParam, ParamKind, ParamValue, Widget};

const MAX_TEXT: usize = 1_000;

/// The widget's single statement when it classifies as a read.
///
/// # Errors
/// [`BoardError::Sql`] for several statements or anything but a read.
pub fn admit_sql(sql: &str, dialect: Dialect) -> Result<String, BoardError> {
    let statements = split(sql, dialect);
    let [statement] = statements.as_slice() else {
        return Err(sql_error("A widget runs exactly one statement"));
    };
    let text = statement.text(sql).trim().to_owned();
    if classify(&text, dialect).kind != StatementKind::Read {
        return Err(sql_error("Widgets run read-only queries only"));
    }
    Ok(text)
}

fn sql_error(message: &str) -> BoardError {
    BoardError::Sql(message.to_owned())
}

/// Bind `widget`'s statement with the board's typed parameters.
///
/// # Errors
/// [`BoardError`] when the SQL is not one read, names an undeclared
/// parameter, or a value is missing or does not match its type.
pub fn bind_widget(
    board: &Board,
    widget: &Widget,
    dialect: Dialect,
    values: &[ParamValue],
) -> Result<BoundSql, BoardError> {
    let sql = admit_sql(&widget.sql, dialect)?;
    let mut binder = ParamBinder::new(dialect, ParamStyle::Named);
    let slots = binder.slots(&sql);
    let resolved = slots
        .iter()
        .map(|slot| resolve(board, values, slot))
        .collect::<Result<Vec<_>, _>>()?;
    let value = |name: &str| {
        let index = slots.iter().position(|slot| slot.name == name)?;
        resolved.get(index).cloned().map(Some)
    };
    binder
        .bind(&sql, &slots, value)
        .map_err(|missing| BoardError::Param(format!("No value for {}", missing.0)))
}

fn resolve(board: &Board, values: &[ParamValue], slot: &ParamSlot) -> Result<String, BoardError> {
    let key = param_key(&slot.name);
    let Some(param) = board.params.iter().find(|param| param.name == key) else {
        return Err(BoardError::Param(format!(
            "Undeclared parameter {}",
            slot.name
        )));
    };
    let entered = values
        .iter()
        .find(|value| value.name == key)
        .and_then(|value| value.value.as_deref())
        .filter(|value| !value.is_empty());
    let Some(value) = entered.or(param.default.as_deref()) else {
        return Err(BoardError::Param(format!("Enter a value for {key}")));
    };
    valid_value(param, value)
}

/// The parameter name of a placeholder: `:id`, `$id`, `@id` or `{id:Type}`.
fn param_key(slot: &str) -> &str {
    if let Some(inner) = slot.strip_prefix('{') {
        return inner.split(':').next().unwrap_or_default();
    }
    slot.trim_start_matches([':', '$', '@'])
}

/// `value` checked against `param`'s type, normalised for binding.
///
/// # Errors
/// [`BoardError::Param`] when the value does not match.
pub fn valid_value(param: &BoardParam, value: &str) -> Result<String, BoardError> {
    let mismatch = |what: &str| BoardError::Param(format!("{} must be {what}", param.name));
    match param.kind {
        ParamKind::Text if value.chars().count() <= MAX_TEXT && !value.contains('\0') => {
            Ok(value.to_owned())
        }
        ParamKind::Text => Err(mismatch("text of at most 1,000 characters")),
        ParamKind::Number => number(value.trim()).ok_or_else(|| mismatch("a finite number")),
        ParamKind::Date => date(value.trim()).ok_or_else(|| mismatch("a date YYYY-MM-DD")),
        ParamKind::Enum if param.options.iter().any(|option| option == value) => {
            Ok(value.to_owned())
        }
        ParamKind::Enum => Err(mismatch("one of its options")),
    }
}

fn number(text: &str) -> Option<String> {
    let plain = !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '+' | '-' | '.' | 'e' | 'E'));
    let parsed = text.parse::<f64>().ok().filter(|value| value.is_finite());
    (plain && parsed.is_some()).then(|| text.to_owned())
}

fn date(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let digits = bytes
        .iter()
        .enumerate()
        .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit());
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' || !digits {
        return None;
    }
    let part = |range: std::ops::Range<usize>| text.get(range)?.parse::<u32>().ok();
    let (year, month, day) = (part(0..4)?, part(5..7)?, part(8..10)?);
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        1..=12 => 31,
        _ => return None,
    };
    (year > 0 && (1..=days).contains(&day)).then(|| text.to_owned())
}
