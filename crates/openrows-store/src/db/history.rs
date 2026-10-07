// SPDX-License-Identifier: Apache-2.0
//! Executed-statement history (docs/15 §2, docs/12 repudiation row).

use rusqlite::{Row, params};

use super::{
    LocalStore,
    error::{StoreError, sqlite},
};

/// Outcome of an executed statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryStatus {
    /// Completed successfully.
    Ok,
    /// Failed with an error.
    Error,
    /// Cancelled by the user.
    Cancelled,
}

/// Which surface issued the statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistorySource {
    /// The SQL editor.
    Editor,
    /// Table browsing.
    Browse,
    /// Committing a grid change set.
    EditCommit,
    /// The API client.
    Api,
}

/// A statement to record; `started_at_ms` is Unix epoch milliseconds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryRecord {
    /// Connection profile the statement ran on.
    pub profile_id: String,
    /// Database selected at the time, if any.
    pub database: Option<String>,
    /// Start time, Unix epoch milliseconds.
    pub started_at_ms: i64,
    /// Wall-clock duration when known.
    pub duration_ms: Option<i64>,
    /// Outcome.
    pub status: HistoryStatus,
    /// Rows returned or affected when known.
    pub rows: Option<i64>,
    /// Statement text as executed.
    pub statement: String,
    /// Error message for failed statements.
    pub error_message: Option<String>,
    /// Issuing surface.
    pub source: HistorySource,
}

/// A recorded statement with its row id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    /// Row id, increasing with insertion order.
    pub id: i64,
    /// The recorded statement.
    pub record: HistoryRecord,
}

impl HistoryStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Error => "error",
            Self::Cancelled => "cancelled",
        }
    }

    fn parse(text: &str) -> Result<Self, StoreError> {
        match text {
            "ok" => Ok(Self::Ok),
            "error" => Ok(Self::Error),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(StoreError::Corrupt {
                column: "history.status",
            }),
        }
    }
}

impl HistorySource {
    fn as_str(self) -> &'static str {
        match self {
            Self::Editor => "editor",
            Self::Browse => "browse",
            Self::EditCommit => "edit-commit",
            Self::Api => "api",
        }
    }

    fn parse(text: &str) -> Result<Self, StoreError> {
        match text {
            "editor" => Ok(Self::Editor),
            "browse" => Ok(Self::Browse),
            "edit-commit" => Ok(Self::EditCommit),
            "api" => Ok(Self::Api),
            _ => Err(StoreError::Corrupt {
                column: "history.source",
            }),
        }
    }
}

const SELECT_RECENT: &str = "SELECT id, profile_id, database, started_at, duration_ms, \
     status, rows, statement, error_message, source FROM history WHERE profile_id = ?1 \
     ORDER BY started_at DESC, id DESC LIMIT ?2";

const INSERT: &str = "INSERT INTO history (profile_id, database, started_at, duration_ms, \
     status, rows, statement, error_message, source) \
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)";

type RawRow = (i64, HistoryRecord, String, String);

fn read_row(row: &Row<'_>) -> rusqlite::Result<RawRow> {
    let record = HistoryRecord {
        profile_id: row.get(1)?,
        database: row.get(2)?,
        started_at_ms: row.get(3)?,
        duration_ms: row.get(4)?,
        status: HistoryStatus::Ok,
        rows: row.get(6)?,
        statement: row.get(7)?,
        error_message: row.get(8)?,
        source: HistorySource::Editor,
    };
    Ok((row.get(0)?, record, row.get(5)?, row.get(9)?))
}

fn into_entry((id, mut record, status, source): RawRow) -> Result<HistoryEntry, StoreError> {
    record.status = HistoryStatus::parse(&status)?;
    record.source = HistorySource::parse(&source)?;
    Ok(HistoryEntry { id, record })
}

impl LocalStore {
    /// Append a statement to history and return its row id.
    ///
    /// # Errors
    /// Returns a `Sqlite` error when the insert fails.
    pub fn record_history(&self, record: &HistoryRecord) -> Result<i64, StoreError> {
        let conn = self.lock()?;
        conn.execute(
            INSERT,
            params![
                record.profile_id,
                record.database,
                record.started_at_ms,
                record.duration_ms,
                record.status.as_str(),
                record.rows,
                record.statement,
                record.error_message,
                record.source.as_str(),
            ],
        )
        .map_err(sqlite("record history"))?;
        Ok(conn.last_insert_rowid())
    }

    /// The newest `limit` history entries for `profile_id`, newest first.
    ///
    /// # Errors
    /// Returns `Sqlite` or `Corrupt` errors when rows cannot be read.
    pub fn recent_history(
        &self,
        profile_id: &str,
        limit: u32,
    ) -> Result<Vec<HistoryEntry>, StoreError> {
        let conn = self.lock()?;
        let mut statement = conn
            .prepare(SELECT_RECENT)
            .map_err(sqlite("read history"))?;
        let rows = statement
            .query_map(params![profile_id, limit], read_row)
            .map_err(sqlite("read history"))?;
        rows.map(|row| into_entry(row.map_err(sqlite("read history"))?))
            .collect()
    }
}
