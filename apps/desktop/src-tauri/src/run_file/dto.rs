// SPDX-License-Identifier: Apache-2.0
//! Request, reply and event types of the `run_file_*` commands.
use openrows_core::safety::{Gate, Verdict};
use openrows_core::script_file::{
    FileEvent, FileOutcome, FileScan, FileStatementError, FileSummary,
};
use serde::{Deserialize, Serialize};
use specta::Type;

#[expect(
    clippy::cast_precision_loss,
    reason = "counts and sizes stay far below 2^53"
)]
fn number(value: u64) -> f64 {
    value as f64
}

/// A `.sql` file chosen in the open panel.
#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PickedScriptDto {
    pub(crate) path: String,
    pub(crate) name: String,
    pub(crate) bytes: f64,
}

/// Check `path` (a picked file) for running on the session's database.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RunFileCheckRequest {
    pub(crate) session_id: String,
    pub(crate) path: String,
}

/// Whether the file may run: `run`, after confirming (`confirm`), or never
/// on this connection (`block`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RunFileGateDto {
    Run,
    Confirm,
    Block,
}

/// A statement that needs confirmation or is blocked.
#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FlaggedStatementDto {
    pub(crate) line: f64,
    pub(crate) preview: String,
    pub(crate) blocked: bool,
}

/// What the file holds and whether it may run.
#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RunFileCheckDto {
    pub(crate) bytes: f64,
    pub(crate) statements: f64,
    pub(crate) gate: RunFileGateDto,
    /// Statements that need confirmation, and those refused.
    pub(crate) confirm: f64,
    pub(crate) blocked: f64,
    /// The first flagged statements, blocked ones first.
    pub(crate) samples: Vec<FlaggedStatementDto>,
    /// Pass back to `run_file_start` once the user confirmed.
    pub(crate) confirm_token: Option<String>,
    /// The engine can run the whole file in one transaction.
    pub(crate) transactions: bool,
}

impl RunFileCheckDto {
    pub(crate) fn new(scan: &FileScan, token: String, transactions: bool) -> Self {
        let gate = match scan.gate() {
            Gate::Run => RunFileGateDto::Run,
            Gate::Confirm => RunFileGateDto::Confirm,
            Gate::Block => RunFileGateDto::Block,
        };
        Self {
            bytes: number(scan.stamp.bytes),
            statements: number(scan.statements),
            gate,
            confirm: number(scan.confirm),
            blocked: number(scan.blocked),
            samples: scan
                .samples
                .iter()
                .map(|sample| FlaggedStatementDto {
                    line: number(sample.line as u64),
                    preview: sample.preview.clone(),
                    blocked: sample.verdict == Verdict::Block,
                })
                .collect(),
            confirm_token: (gate == RunFileGateDto::Confirm).then_some(token),
            transactions,
        }
    }
}

/// Run a checked file on a session of its own.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RunFileStartRequest {
    pub(crate) session_id: String,
    pub(crate) path: String,
    /// End at the first error; otherwise run on and collect the errors.
    pub(crate) stop_on_error: bool,
    /// Run in one transaction, rolled back on error or cancel.
    pub(crate) transaction: bool,
    pub(crate) confirm_token: Option<String>,
}

/// The run started; events follow and `query_cancel(runId)` cancels it.
#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RunFileStarted {
    pub(crate) run_id: String,
}

/// How a file run ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RunFileOutcomeDto {
    Completed,
    Failed,
    Cancelled,
}

/// File run events; `finished` ends them.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum RunFileEventDto {
    /// Counts so far, at most 10 per second; `current` is the running statement.
    Progress {
        statements: f64,
        bytes: f64,
        errors: f64,
        current: String,
    },
    /// A statement failed (the first 1,000 errors are reported).
    Error {
        statement: f64,
        line: f64,
        preview: String,
        message: String,
    },
    Finished {
        outcome: RunFileOutcomeDto,
        statements: f64,
        errors: f64,
        rows_affected: f64,
        rolled_back: bool,
        /// Why the run stopped early, when not a statement error.
        message: Option<String>,
        elapsed_ms: f64,
    },
}

impl From<FileEvent> for RunFileEventDto {
    fn from(event: FileEvent) -> Self {
        match event {
            FileEvent::Progress(progress) => Self::Progress {
                statements: number(progress.statements),
                bytes: number(progress.bytes),
                errors: number(progress.errors),
                current: progress.current,
            },
            FileEvent::Error(FileStatementError {
                statement,
                line,
                preview,
                message,
            }) => Self::Error {
                statement: number(statement),
                line: number(line as u64),
                preview,
                message,
            },
        }
    }
}

/// The terminal event of a run that took `elapsed_ms`.
pub(crate) fn finished(summary: FileSummary, elapsed_ms: u64) -> RunFileEventDto {
    RunFileEventDto::Finished {
        outcome: match summary.outcome {
            FileOutcome::Completed => RunFileOutcomeDto::Completed,
            FileOutcome::Failed => RunFileOutcomeDto::Failed,
            FileOutcome::Cancelled => RunFileOutcomeDto::Cancelled,
        },
        statements: number(summary.statements),
        errors: number(summary.errors),
        rows_affected: number(summary.rows_affected),
        rolled_back: summary.rolled_back,
        message: summary.message,
        elapsed_ms: number(elapsed_ms),
    }
}
