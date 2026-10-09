// SPDX-License-Identifier: Apache-2.0
//! Request and reply types of `query_execute` and the `history_*` commands.
use super::params::ParamsDto;
use openrows_core::safety::{SafetyPlan, Verdict};
use openrows_core::script::{self, Scope};
use openrows_sql::{Dialect, StatementKind};
use openrows_store::{HistoryEntry, HistoryStatus};
use serde::{Deserialize, Serialize};
use specta::Type;

/// Which statements of the editor text to run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RunMode {
    /// Every statement in `sql`, in order.
    All,
    /// Every statement in `sql`, which holds only the selected text.
    Selection,
    /// The statement at `cursor` (or the closest one before it).
    Current,
}

/// Run SQL on an open session.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExecuteRequest {
    pub(crate) session_id: String,
    pub(crate) sql: String,
    pub(crate) mode: RunMode,
    /// Editor cursor as a UTF-16 offset into `sql`, for `current`.
    pub(crate) cursor: Option<u32>,
    /// Token from a `confirmationRequired` reply the user accepted.
    pub(crate) confirm_token: Option<String>,
    /// Rows kept per result set (the editor's row limit); `None` keeps the
    /// result store's default cap.
    #[serde(default)]
    pub(crate) max_rows: Option<u32>,
    /// Stop at the first failed statement ("Return on Error", the default).
    #[serde(default = "stop_on_error")]
    pub(crate) return_on_error: bool,
    /// Parameter placeholders to bind; `None` runs the text as written.
    #[serde(default)]
    pub(crate) params: Option<ParamsDto>,
}

const fn stop_on_error() -> bool {
    true
}

impl ExecuteRequest {
    /// Statement texts the request selects, in execution order, split with
    /// the session engine's `dialect`.
    pub(crate) fn statements(&self, dialect: Dialect) -> Vec<String> {
        let scope = match self.mode {
            RunMode::All | RunMode::Selection => Scope::All,
            RunMode::Current => {
                let cursor = self.cursor.map_or(0, |cursor| cursor as usize);
                Scope::AtCursor(byte_offset(&self.sql, cursor))
            }
        };
        script::statements(&self.sql, scope, dialect)
    }

    /// Rows kept per result set of this run.
    pub(crate) fn row_cap(&self) -> Option<usize> {
        self.max_rows.and_then(|rows| usize::try_from(rows).ok())
    }
}

/// Byte offset of a UTF-16 offset (`CodeMirror` positions), clamped to the text.
pub(crate) fn byte_offset(text: &str, utf16: usize) -> usize {
    let mut units = 0;
    for (index, character) in text.char_indices() {
        if units >= utf16 {
            return index;
        }
        units += character.len_utf16();
    }
    text.len()
}

/// What a statement does, for the confirmation dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum StatementKindDto {
    Read,
    Write,
    Ddl,
    TxControl,
    Unknown,
}

/// The safety gate's decision for one statement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum VerdictDto {
    Allow,
    Confirm,
    Block,
}

/// One statement of the run with its classification.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StatementDto {
    pub(crate) index: u32,
    pub(crate) sql: String,
    pub(crate) kind: StatementKindDto,
    pub(crate) dangerous: bool,
    pub(crate) verdict: VerdictDto,
}

/// Every statement of a plan, in execution order.
pub(crate) fn statement_dtos(plan: &SafetyPlan) -> Vec<StatementDto> {
    plan.statements
        .iter()
        .enumerate()
        .map(|(index, statement)| StatementDto {
            index: u32::try_from(index).unwrap_or(u32::MAX),
            sql: statement.sql.clone(),
            kind: kind(statement.class.kind),
            dangerous: statement.class.dangerous,
            verdict: match statement.verdict {
                Verdict::Allow => VerdictDto::Allow,
                Verdict::Confirm => VerdictDto::Confirm,
                Verdict::Block => VerdictDto::Block,
            },
        })
        .collect()
}

fn kind(kind: StatementKind) -> StatementKindDto {
    match kind {
        StatementKind::Read => StatementKindDto::Read,
        StatementKind::Write => StatementKindDto::Write,
        StatementKind::Ddl => StatementKindDto::Ddl,
        StatementKind::TxControl => StatementKindDto::TxControl,
        _ => StatementKindDto::Unknown,
    }
}

/// How `query_execute` answered.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum ExecuteReply {
    /// The run started; events follow on the channel.
    Started {
        query_id: String,
        statements: Vec<StatementDto>,
    },
    /// Nothing ran: show the statements and resend with `confirmToken`.
    ConfirmationRequired {
        token: String,
        statements: Vec<StatementDto>,
    },
    /// Nothing ran: the connection is read-only.
    Blocked {
        message: String,
        statements: Vec<StatementDto>,
    },
    /// Nothing ran: resend with a value for each parameter name.
    ParametersRequired { names: Vec<String> },
}

/// Outcome of a recorded statement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum HistoryStatusDto {
    Ok,
    Error,
    Cancelled,
}

/// Search of the local history: one connection or all, and free text.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HistorySearchRequest {
    /// The saved connection to search; `null` searches every connection.
    pub(crate) profile_id: Option<String>,
    /// Words that must each start a word of the statement; blank lists everything.
    pub(crate) text: String,
    /// Entries wanted, newest first (capped by the command).
    pub(crate) limit: u32,
}

/// One executed statement from the local history.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HistoryItemDto {
    id: f64,
    profile_id: String,
    database: Option<String>,
    statement: String,
    started_at_ms: f64,
    duration_ms: Option<f64>,
    status: HistoryStatusDto,
    rows: Option<f64>,
    error_message: Option<String>,
}

#[expect(
    clippy::cast_precision_loss,
    reason = "ids, epoch milliseconds and row counts stay far below 2^53"
)]
impl From<HistoryEntry> for HistoryItemDto {
    fn from(entry: HistoryEntry) -> Self {
        let record = entry.record;
        Self {
            id: entry.id as f64,
            profile_id: record.profile_id,
            database: record.database,
            statement: record.statement,
            started_at_ms: record.started_at_ms as f64,
            duration_ms: record.duration_ms.map(|ms| ms as f64),
            status: match record.status {
                HistoryStatus::Ok => HistoryStatusDto::Ok,
                HistoryStatus::Error => HistoryStatusDto::Error,
                HistoryStatus::Cancelled => HistoryStatusDto::Cancelled,
            },
            rows: record.rows.map(|rows| rows as f64),
            error_message: record.error_message,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(sql: &str, mode: RunMode, cursor: Option<u32>) -> ExecuteRequest {
        ExecuteRequest {
            session_id: String::new(),
            sql: sql.to_owned(),
            mode,
            cursor,
            confirm_token: None,
            max_rows: None,
            return_on_error: true,
            params: None,
        }
    }

    #[test]
    fn the_row_limit_is_optional_on_the_wire() -> Result<(), serde_json::Error> {
        let limited: ExecuteRequest = serde_json::from_value(serde_json::json!({
            "sessionId": "s", "sql": "SELECT 1", "mode": "all",
            "cursor": null, "confirmToken": null, "maxRows": 500
        }))?;
        assert_eq!(limited.row_cap(), Some(500));
        let open: ExecuteRequest = serde_json::from_value(serde_json::json!({
            "sessionId": "s", "sql": "SELECT 1", "mode": "all",
            "cursor": null, "confirmToken": null
        }))?;
        assert_eq!(open.row_cap(), None);
        assert!(open.return_on_error && open.params.is_none());
        Ok(())
    }

    #[test]
    fn utf16_cursors_map_to_byte_offsets() {
        let text = "é😀a";
        assert_eq!(byte_offset(text, 0), 0);
        assert_eq!(byte_offset(text, 1), 2);
        assert_eq!(byte_offset(text, 3), 6);
        assert_eq!(byte_offset(text, 4), 7);
        assert_eq!(byte_offset(text, 99), 7);
    }

    #[test]
    fn modes_select_statements() {
        let sql = "SELECT 'ü';\nSELECT 2;\nSELECT 3";
        assert_eq!(
            request(sql, RunMode::All, None)
                .statements(Dialect::Postgres)
                .len(),
            3
        );
        assert_eq!(
            request("SELECT 1; SELECT 2", RunMode::Selection, None)
                .statements(Dialect::Postgres)
                .len(),
            2
        );
        let second = u32::try_from("SELECT 'ü';\nSEL".encode_utf16().count()).unwrap_or(0);
        assert_eq!(
            request(sql, RunMode::Current, Some(second)).statements(Dialect::Postgres),
            vec!["SELECT 2".to_owned()]
        );
        assert_eq!(
            request(sql, RunMode::Current, None).statements(Dialect::Postgres),
            vec!["SELECT 'ü'".to_owned()]
        );
        assert!(
            request(" -- nothing ", RunMode::All, None)
                .statements(Dialect::Postgres)
                .is_empty()
        );
    }

    #[test]
    fn replies_serialize_with_a_kind_tag() -> Result<(), serde_json::Error> {
        let reply = ExecuteReply::ConfirmationRequired {
            token: "t".to_owned(),
            statements: Vec::new(),
        };
        let json = serde_json::to_value(reply)?;
        assert_eq!(json["kind"], "confirmationRequired");
        assert_eq!(json["token"], "t");
        Ok(())
    }
}
