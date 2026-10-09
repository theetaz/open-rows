// SPDX-License-Identifier: Apache-2.0
//! Counting a file run's statement events into progress and error reports.
use super::run::{FileOutcome, FileSummary};
use super::scan::preview;
use crate::QueryEvent;
use crate::error::ErrorKind;
use crate::sink::{PROGRESS_INTERVAL, Throttle};
use openrows_sql::ScriptStatement;
use std::time::Instant;

/// Errors reported one by one; later ones are only counted.
const MAX_REPORTED: u64 = 1_000;

/// What a running file reports.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FileEvent {
    /// Counts so far; at most 10 per second, plus one at the end.
    Progress(FileProgress),
    /// A statement failed (the first 1,000 errors).
    Error(FileStatementError),
}

/// Progress of a file run.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FileProgress {
    /// Statements that ran (succeeded or failed).
    pub statements: u64,
    /// Bytes of the file up to the end of the last statement that ran.
    pub bytes: u64,
    /// Statements that failed.
    pub errors: u64,
    /// The running statement, folded to one line.
    pub current: String,
}

/// One failed statement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileStatementError {
    /// 1-based position of the statement in the file.
    pub statement: u64,
    /// 1-based line where it starts.
    pub line: usize,
    /// Its first characters on one line.
    pub preview: String,
    /// The server's message.
    pub message: String,
}

pub(super) struct Tally<E> {
    emit: E,
    throttle: Throttle,
    progress: FileProgress,
    /// Statements before the current batch.
    base: u64,
    rows_affected: u64,
    lost: bool,
}

impl<E: FnMut(FileEvent)> Tally<E> {
    pub(super) fn new(emit: E) -> Self {
        Self {
            emit,
            throttle: Throttle::new(PROGRESS_INTERVAL),
            progress: FileProgress::default(),
            base: 0,
            rows_affected: 0,
            lost: false,
        }
    }

    pub(super) fn start_batch(&mut self) {
        self.base = self.progress.statements;
    }

    pub(super) fn ran(&self) -> u64 {
        self.progress.statements
    }

    pub(super) fn errors(&self) -> u64 {
        self.progress.errors
    }

    /// The connection was lost: later statements must not run.
    pub(super) fn lost(&self) -> bool {
        self.lost
    }

    /// Count one event of the batch's `index`th statement.
    pub(super) fn observe(
        &mut self,
        (index, statement): (usize, &ScriptStatement),
        event: &QueryEvent,
    ) {
        match event {
            QueryEvent::Started { .. } => {
                self.progress.current = preview(&statement.text);
                self.tick(false);
            }
            QueryEvent::RowsAffected { count } => self.rows_affected += count,
            QueryEvent::Completed { .. } => self.ran_one(statement),
            QueryEvent::Failed { error } => {
                self.lost |= matches!(error.kind, ErrorKind::Network | ErrorKind::SessionClosed);
                self.ran_one(statement);
                self.progress.errors += 1;
                if self.progress.errors <= MAX_REPORTED {
                    (self.emit)(FileEvent::Error(FileStatementError {
                        statement: self.base + index as u64 + 1,
                        line: statement.line,
                        preview: preview(&statement.text),
                        message: error.message.clone(),
                    }));
                }
            }
            _ => {}
        }
    }

    fn ran_one(&mut self, statement: &ScriptStatement) {
        self.progress.statements += 1;
        self.progress.bytes = statement.end;
        self.tick(false);
    }

    fn tick(&mut self, force: bool) {
        if force || self.throttle.ready(Instant::now()) {
            (self.emit)(FileEvent::Progress(self.progress.clone()));
        }
    }

    /// The final progress report and the summary.
    pub(super) fn summary(
        mut self,
        outcome: FileOutcome,
        rolled_back: bool,
        message: Option<String>,
    ) -> FileSummary {
        self.progress.current.clear();
        self.tick(true);
        FileSummary {
            outcome,
            statements: self.progress.statements,
            errors: self.progress.errors,
            rows_affected: self.rows_affected,
            rolled_back,
            message,
        }
    }
}
