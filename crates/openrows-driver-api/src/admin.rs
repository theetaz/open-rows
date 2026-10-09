// SPDX-License-Identifier: Apache-2.0
//! Server activity: the process list and cancel/terminate (FR-ADMIN-01,
//! docs/26 A01–A03).
//!
//! The [`Admin`] facet runs on a connection of its own (the app uses the
//! per-database metadata connection), never on the user's query session, so
//! the list still answers while a user statement runs.
use crate::{BoxFuture, DriverError};

/// A server process (`PostgreSQL` backend pid, `MySQL` connection id).
pub type ProcessId = u64;

/// One session or process on the server, as the process list shows it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProcessInfo {
    /// Backend pid or connection id; what [`Admin::signal_process`] takes.
    pub id: ProcessId,
    /// Login role or user.
    pub user: Option<String>,
    /// Database the process is connected to.
    pub database: Option<String>,
    /// Client address (and port when known); `None` for local sockets.
    pub client: Option<String>,
    /// Application name the client reported, e.g. `psql` or `OpenRows`.
    pub application: Option<String>,
    /// What the process is doing, e.g. `active`, `idle`, `Query`, `Sleep`.
    pub state: Option<String>,
    /// What it waits for, e.g. `Lock: relation` or `User sleep`.
    pub wait: Option<String>,
    /// When the current (or last) statement started, as server text.
    pub query_start: Option<String>,
    /// How long the current state or statement has run, in milliseconds.
    pub duration_ms: Option<u64>,
    /// The current (or last) statement text.
    pub query: Option<String>,
    /// The connection this list was read on.
    pub current: bool,
}

/// What to do to a process.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessSignal {
    /// Abort the running statement; the session stays connected
    /// (`pg_cancel_backend`, `KILL QUERY`).
    CancelQuery,
    /// End the session, rolling back its open transaction
    /// (`pg_terminate_backend`, `KILL`).
    Terminate,
}

/// Process listing and signalling (docs/05 §2: admin facet).
pub trait Admin: Send {
    /// The server process of this connection.
    ///
    /// # Errors
    /// Server or transport failures.
    fn own_process(&mut self) -> BoxFuture<'_, Result<ProcessId, DriverError>>;

    /// Client sessions on the server the login may see, the longest-running
    /// first; this connection is marked [`ProcessInfo::current`].
    ///
    /// # Errors
    /// Server or transport failures.
    fn list_processes(&mut self) -> BoxFuture<'_, Result<Vec<ProcessInfo>, DriverError>>;

    /// Cancel the statement of process `id`, or terminate it.
    ///
    /// # Errors
    /// `Server` when the process is unknown or already gone (SQLSTATE
    /// `42704`), or when the login lacks the privilege; transport failures.
    fn signal_process(
        &mut self,
        id: ProcessId,
        signal: ProcessSignal,
    ) -> BoxFuture<'_, Result<(), DriverError>>;
}

/// The error for a process the server does not know (already ended).
#[must_use]
pub fn unknown_process(id: ProcessId) -> DriverError {
    DriverError::Server {
        code: "42704".to_owned(),
        message: format!("process {id} was not found; it may have ended already"),
    }
}

impl ProcessInfo {
    /// A process from one text row whose columns are, in order: id, user,
    /// database, client, application, state, wait, query start, duration
    /// in milliseconds, query and current (`t`, `1` or `true`). Drivers
    /// write their listing SQL to this shape; empty text is `None`.
    ///
    /// # Errors
    /// `Protocol` when the id or duration is not a non-negative integer.
    pub fn from_text_row(row: &[Option<String>]) -> Result<Self, DriverError> {
        let text = |index: usize| cell_text(row, index);
        Ok(Self {
            id: cell_number(row, 0)?
                .ok_or_else(|| DriverError::Protocol("process without id".to_owned()))?,
            user: text(1),
            database: text(2),
            client: text(3),
            application: text(4),
            state: text(5),
            wait: text(6),
            query_start: text(7),
            duration_ms: cell_number(row, 8)?,
            query: text(9),
            current: matches!(text(10).as_deref(), Some("t" | "1" | "true")),
        })
    }
}

/// Column `index` of a text row; NULL and empty text are `None`.
fn cell_text(row: &[Option<String>], index: usize) -> Option<String> {
    row.get(index)
        .cloned()
        .flatten()
        .filter(|value| !value.is_empty())
}

/// Column `index` as a non-negative integer; `None` when NULL or empty.
fn cell_number(row: &[Option<String>], index: usize) -> Result<Option<u64>, DriverError> {
    let Some(value) = cell_text(row, index) else {
        return Ok(None);
    };
    value
        .parse()
        .map(Some)
        .map_err(|_| DriverError::Protocol(format!("process list column {index}: {value}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(cells: &[Option<&str>]) -> Vec<Option<String>> {
        cells.iter().map(|cell| cell.map(str::to_owned)).collect()
    }

    #[test]
    fn text_rows_become_processes() -> Result<(), DriverError> {
        let process = ProcessInfo::from_text_row(&row(&[
            Some("4711"),
            Some("app"),
            Some("orders"),
            Some("10.0.0.5:51234"),
            Some(""),
            Some("active"),
            None,
            Some("2026-10-09 10:00:00+00"),
            Some("1500"),
            Some("SELECT pg_sleep(60)"),
            Some("t"),
        ]))?;
        assert_eq!(process.id, 4711);
        assert_eq!(process.application, None);
        assert_eq!(process.duration_ms, Some(1500));
        assert!(process.current);
        let short = ProcessInfo::from_text_row(&row(&[Some("7"), None]))?;
        assert!(!short.current && short.query.is_none());
        Ok(())
    }

    #[test]
    fn malformed_ids_and_durations_are_protocol_errors() {
        for cells in [
            row(&[None]),
            row(&[Some("x")]),
            row(&[
                Some("1"),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some("-5"),
            ]),
        ] {
            assert!(matches!(
                ProcessInfo::from_text_row(&cells),
                Err(DriverError::Protocol(_))
            ));
        }
    }

    #[test]
    fn unknown_processes_are_server_errors_naming_the_id() {
        let error = unknown_process(42);
        assert!(!error.is_connection_lost());
        assert!(error.to_string().contains("process 42 was not found"));
        assert!(error.to_string().contains("42704"));
    }
}
