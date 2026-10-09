// SPDX-License-Identifier: Apache-2.0
//! Executed-statement history (docs/15 §2, docs/12 repudiation row).

use rusqlite::{Connection, Row, ToSql, params};

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

/// Newest first; `?1` filters by profile when not NULL.
const LIST: &str = "SELECT h.id, h.profile_id, h.database, h.started_at, h.duration_ms, \
     h.status, h.rows, h.statement, h.error_message, h.source FROM history h \
     WHERE ?1 IS NULL OR h.profile_id = ?1 \
     ORDER BY h.started_at DESC, h.id DESC LIMIT ?2";

/// Newest first statements matching the FTS5 expression in `?1`.
const SEARCH: &str = "SELECT h.id, h.profile_id, h.database, h.started_at, h.duration_ms, \
     h.status, h.rows, h.statement, h.error_message, h.source \
     FROM history_fts JOIN history h ON h.id = history_fts.rowid \
     WHERE history_fts MATCH ?1 AND (?2 IS NULL OR h.profile_id = ?2) \
     ORDER BY h.started_at DESC, h.id DESC LIMIT ?3";

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

/// An FTS5 expression for free text: every whitespace-separated term must
/// match as a prefix. Terms are quoted, so punctuation such as the dot in
/// `public.orders` or a stray quote is never read as FTS syntax. `None` when
/// the text holds no term, which means "no search".
pub(crate) fn fts_query(text: &str) -> Option<String> {
    let terms: Vec<String> = text
        .split_whitespace()
        .map(|term| format!("\"{}\"*", term.replace('"', "\"\"")))
        .collect();
    (!terms.is_empty()).then(|| terms.join(" "))
}

fn collect(
    conn: &Connection,
    sql: &str,
    params: &[&dyn ToSql],
) -> Result<Vec<HistoryEntry>, StoreError> {
    let mut statement = conn.prepare(sql).map_err(sqlite("read history"))?;
    let rows = statement
        .query_map(params, read_row)
        .map_err(sqlite("read history"))?;
    rows.map(|row| into_entry(row.map_err(sqlite("read history"))?))
        .collect()
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

    /// Up to `limit` entries newest first, of one profile or of every profile
    /// (`None`). Non-blank `text` keeps only statements that contain all of its
    /// words as prefixes (full-text search, FR-WS-01).
    ///
    /// # Errors
    /// Returns `Sqlite` or `Corrupt` errors when rows cannot be read.
    pub fn search_history(
        &self,
        profile_id: Option<&str>,
        text: &str,
        limit: u32,
    ) -> Result<Vec<HistoryEntry>, StoreError> {
        let conn = self.lock()?;
        match fts_query(text) {
            None => collect(&conn, LIST, params![profile_id, limit]),
            Some(expression) => collect(&conn, SEARCH, params![expression, profile_id, limit]),
        }
    }

    /// Delete one entry; `false` when no entry has that id.
    ///
    /// # Errors
    /// Returns a `Sqlite` error when the delete fails.
    pub fn delete_history(&self, id: i64) -> Result<bool, StoreError> {
        let removed = self
            .lock()?
            .execute("DELETE FROM history WHERE id = ?1", [id])
            .map_err(sqlite("delete history"))?;
        Ok(removed > 0)
    }

    /// Delete every entry of `profile_id`; returns how many were removed.
    ///
    /// # Errors
    /// Returns a `Sqlite` error when the delete fails.
    pub fn clear_history(&self, profile_id: &str) -> Result<usize, StoreError> {
        self.lock()?
            .execute("DELETE FROM history WHERE profile_id = ?1", [profile_id])
            .map_err(sqlite("clear history"))
    }
}

#[cfg(test)]
mod tests {
    use super::fts_query;

    #[test]
    fn terms_are_quoted_prefixes_that_must_all_match() {
        assert_eq!(fts_query("   "), None);
        assert_eq!(
            fts_query("select public.orders").as_deref(),
            Some("\"select\"* \"public.orders\"*")
        );
        assert_eq!(fts_query("a\"b").as_deref(), Some("\"a\"\"b\"*"));
    }
}
