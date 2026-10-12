// SPDX-License-Identifier: Apache-2.0
//! Size, version and schema checks of board documents; parsing never runs SQL.
use std::collections::HashSet;

use serde::Deserialize;

use super::BoardError;
use super::bind::valid_value;
use super::model::{BOARD_VERSION, Board, BoardParam, ParamKind, Widget, WidgetKind, WidgetLimits};

/// Largest accepted board document, in bytes.
pub const MAX_BOARD_BYTES: usize = 256 * 1024;
const MAX_WIDGETS: usize = 24;
const MAX_PARAMS: usize = 16;
const MAX_OPTIONS: usize = 50;
const MAX_SERIES: usize = 8;
const MAX_NAME: usize = 120;
/// Longest widget statement, in bytes.
pub const MAX_SQL: usize = 16_384;
/// Accepted interval refresh periods, in seconds.
pub const REFRESH_SECS: [u32; 4] = [30, 60, 300, 900];

#[derive(Deserialize)]
struct Header {
    version: u32,
}

/// Parse and validate an imported document.
///
/// # Errors
/// [`BoardError`] when the text is oversize, malformed, of another version
/// or fails [`validate`].
pub fn parse_board(text: &str) -> Result<Board, BoardError> {
    if text.len() > MAX_BOARD_BYTES {
        return Err(BoardError::TooLarge(MAX_BOARD_BYTES));
    }
    let header: Header = serde_json::from_str(text).map_err(malformed)?;
    if header.version != BOARD_VERSION {
        return Err(BoardError::Version(header.version));
    }
    let board: Board = serde_json::from_str(text).map_err(malformed)?;
    validate(&board)?;
    Ok(board)
}

/// Pretty JSON of a validated board, for export.
///
/// # Errors
/// [`BoardError`] when the board is invalid or larger than the import limit.
pub fn export_board(board: &Board) -> Result<String, BoardError> {
    validate(board)?;
    let text = serde_json::to_string_pretty(board).map_err(malformed)?;
    if text.len() > MAX_BOARD_BYTES {
        return Err(BoardError::TooLarge(MAX_BOARD_BYTES));
    }
    Ok(text)
}

#[expect(clippy::needless_pass_by_value, reason = "used as a map_err function")]
fn malformed(error: serde_json::Error) -> BoardError {
    BoardError::Malformed(error.to_string())
}

/// Structural checks shared by save, import and export.
///
/// # Errors
/// [`BoardError::Invalid`] naming the first problem.
pub fn validate(board: &Board) -> Result<(), BoardError> {
    if board.version != BOARD_VERSION {
        return Err(BoardError::Version(board.version));
    }
    name(&board.name, "Board name")?;
    if let Some(secs) = board.refresh_secs
        && !REFRESH_SECS.contains(&secs)
    {
        return Err(invalid("Refresh interval must be 30 s, 1, 5 or 15 min"));
    }
    if board.params.len() > MAX_PARAMS || board.widgets.len() > MAX_WIDGETS {
        return Err(invalid("A board has at most 16 parameters and 24 widgets"));
    }
    unique(
        board.params.iter().map(|param| param.name.as_str()),
        "parameter",
    )?;
    unique(
        board.widgets.iter().map(|widget| widget.id.as_str()),
        "widget id",
    )?;
    board.params.iter().try_for_each(param)?;
    board.widgets.iter().try_for_each(widget)
}

fn invalid(message: &str) -> BoardError {
    BoardError::Invalid(message.to_owned())
}

fn name(text: &str, what: &str) -> Result<(), BoardError> {
    if text.trim().is_empty() || text.chars().count() > MAX_NAME {
        return Err(BoardError::Invalid(format!(
            "{what} needs 1 to {MAX_NAME} characters"
        )));
    }
    Ok(())
}

fn unique<'a>(items: impl Iterator<Item = &'a str>, what: &str) -> Result<(), BoardError> {
    let mut seen = HashSet::new();
    for item in items {
        if !seen.insert(item) {
            return Err(BoardError::Invalid(format!("Duplicate {what} {item}")));
        }
    }
    Ok(())
}

/// Whether `text` is a SQL-safe identifier of 1–63 bytes.
pub(super) fn identifier(text: &str) -> bool {
    let mut chars = text.chars();
    let starts = chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_');
    starts && text.len() <= 63 && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn param(param: &BoardParam) -> Result<(), BoardError> {
    if !identifier(&param.name) {
        return Err(BoardError::Invalid(format!(
            "Parameter {} must be a letter or _ followed by letters, digits or _",
            param.name
        )));
    }
    let enumerated = param.kind == ParamKind::Enum;
    if enumerated && !(1..=MAX_OPTIONS).contains(&param.options.len()) {
        return Err(invalid("An enum parameter needs 1 to 50 options"));
    }
    if !enumerated && !param.options.is_empty() {
        return Err(invalid("Only enum parameters have options"));
    }
    if param
        .options
        .iter()
        .any(|option| option.chars().count() > 200)
    {
        return Err(invalid("Enum options have at most 200 characters"));
    }
    match &param.default {
        Some(value) => valid_value(param, value).map(drop),
        None => Ok(()),
    }
}

fn widget(widget: &Widget) -> Result<(), BoardError> {
    let id_ok = !widget.id.is_empty()
        && widget.id.len() <= 64
        && widget
            .id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !id_ok {
        return Err(invalid("Widget ids have 1 to 64 letters, digits, - or _"));
    }
    name(&widget.title, "Widget title")?;
    if widget.sql.trim().is_empty() || widget.sql.len() > MAX_SQL || widget.sql.contains('\0') {
        return Err(invalid("Widget SQL needs 1 to 16,384 bytes"));
    }
    limits(widget.limits)?;
    match (&widget.chart, widget.kind) {
        (Some(chart), WidgetKind::Chart) => {
            let columns = std::iter::once(&chart.x).chain(&chart.y);
            let bad = columns
                .clone()
                .any(|c| c.trim().is_empty() || c.len() > 128);
            if bad || !(1..=MAX_SERIES).contains(&chart.y.len()) {
                return Err(invalid("A chart maps one x column and 1 to 8 y columns"));
            }
            Ok(())
        }
        (None, WidgetKind::Chart) => Err(invalid("A chart widget needs a column mapping")),
        (Some(_), _) => Err(invalid("Only chart widgets have a column mapping")),
        (None, _) => Ok(()),
    }
}

fn limits(limits: WidgetLimits) -> Result<(), BoardError> {
    let ok = (1..=10_000).contains(&limits.max_rows)
        && (1_000..=60_000).contains(&limits.timeout_ms)
        && (1_024..=8 * 1_048_576).contains(&limits.max_bytes);
    if ok {
        Ok(())
    } else {
        Err(invalid(
            "Limits: 1–10,000 rows, 1–60 s and 1 KiB–8 MiB per widget",
        ))
    }
}
