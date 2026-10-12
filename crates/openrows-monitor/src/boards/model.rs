// SPDX-License-Identifier: Apache-2.0
//! Portable, secret-free board documents (docs/28 §5, FR-ADMIN-07).
//!
//! Unknown fields are rejected, so an imported document cannot smuggle
//! approval state, connection secrets or executable content.
use serde::{Deserialize, Serialize};

/// The only board document version this build reads and writes.
pub const BOARD_VERSION: u32 = 1;

/// A named set of widgets sharing typed parameters.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "ipc", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Board {
    /// Document version; always [`BOARD_VERSION`].
    pub version: u32,
    /// Display name.
    pub name: String,
    /// Typed inputs referenced as `:name` in widget SQL.
    pub params: Vec<BoardParam>,
    /// Widgets in display order.
    pub widgets: Vec<Widget>,
    /// Interval refresh in seconds; `None` refreshes manually only.
    pub refresh_secs: Option<u32>,
}

/// One typed board input.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "ipc", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BoardParam {
    /// Identifier used as `:name` in SQL.
    pub name: String,
    /// Value type, checked before binding.
    pub kind: ParamKind,
    /// Value used when the user entered none.
    pub default: Option<String>,
    /// Allowed values of an `enum` parameter.
    pub options: Vec<String>,
}

/// Value type of a board parameter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "ipc", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ParamKind {
    /// Free text, bound as a string.
    Text,
    /// A finite decimal number.
    Number,
    /// A calendar date, `YYYY-MM-DD`.
    Date,
    /// One of the declared options.
    Enum,
}

/// A query-backed visual.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "ipc", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Widget {
    /// Stable identifier within the board.
    pub id: String,
    /// Display title.
    pub title: String,
    /// How the result is shown.
    pub kind: WidgetKind,
    /// One read-only statement; parameters as `:name`.
    pub sql: String,
    /// Column mapping of a chart widget.
    pub chart: Option<ChartMapping>,
    /// Row/time/byte bounds; defaults when absent.
    pub limits: WidgetLimits,
}

/// Presentation of a widget's result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "ipc", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum WidgetKind {
    /// Line, area, bar or scatter chart.
    Chart,
    /// The first value of the first row.
    Stat,
    /// The result rows.
    Table,
}

/// Explicit column mapping of a chart.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "ipc", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChartMapping {
    /// Mark type.
    pub mark: Mark,
    /// Column on the x axis (time, number or category).
    pub x: String,
    /// Numeric series columns.
    pub y: Vec<String>,
}

/// Chart mark type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "ipc", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum Mark {
    /// Connected points.
    Line,
    /// Filled line.
    Area,
    /// Bars per x value.
    Bar,
    /// Unconnected points.
    Scatter,
}

/// Resource bounds of one widget run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "ipc", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WidgetLimits {
    /// Rows kept; more rows stop the stream.
    pub max_rows: u32,
    /// Deadline of the whole run, in milliseconds.
    pub timeout_ms: u32,
    /// Cell text kept, in bytes.
    pub max_bytes: u32,
}

impl Default for WidgetLimits {
    fn default() -> Self {
        Self {
            max_rows: 1_000,
            timeout_ms: 15_000,
            max_bytes: 1_048_576,
        }
    }
}

/// A value the user entered for one parameter; `None` uses the default.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "ipc", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ParamValue {
    /// Parameter name, without sigil.
    pub name: String,
    /// Entered text.
    pub value: Option<String>,
}
