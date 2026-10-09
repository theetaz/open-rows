// SPDX-License-Identifier: Apache-2.0
//! The process list (FR-ADMIN-01): `information_schema.PROCESSLIST` (both
//! `MySQL` and `MariaDB`), `KILL QUERY` and `KILL`. The client's program
//! name comes from `performance_schema.session_connect_attrs` when the
//! login may read it, else the list goes without it.
use openrows_driver_api::admin::unknown_process;
use openrows_driver_api::{Admin, BoxFuture, DriverError, ProcessId, ProcessInfo, ProcessSignal};

use crate::session::MySqlSession;

/// Columns in [`ProcessInfo::from_text_row`] order; `{program}` is the
/// program-name expression and `{join}` its join.
const PROCESSES_SQL: &str = "\
SELECT p.ID, p.USER, p.DB, p.HOST, {program}, p.COMMAND, p.STATE, \
    DATE_FORMAT(NOW() - INTERVAL p.TIME SECOND, '%Y-%m-%d %H:%i:%s'), \
    p.TIME * 1000, p.INFO, p.ID = CONNECTION_ID() \
FROM information_schema.PROCESSLIST p{join} \
WHERE p.COMMAND <> 'Daemon' \
ORDER BY p.TIME DESC, p.ID";

/// The program-name attribute each client sends on connect.
const PROGRAM_JOIN: &str = " LEFT JOIN performance_schema.session_connect_attrs a \
    ON a.PROCESSLIST_ID = p.ID AND a.ATTR_NAME = 'program_name'";

/// `ER_NO_SUCH_THREAD`: `KILL` named a connection that does not exist.
const NO_SUCH_THREAD: &str = "(MySQL error 1094)";

/// The listing, with or without the program names.
fn processes_sql(programs: bool) -> String {
    let (program, join) = if programs {
        ("a.ATTR_VALUE", PROGRAM_JOIN)
    } else {
        ("NULL", "")
    };
    PROCESSES_SQL
        .replace("{program}", program)
        .replace("{join}", join)
}

/// The statement signalling `id`; ids are integers, so no quoting applies.
fn signal_sql(id: ProcessId, signal: ProcessSignal) -> String {
    match signal {
        ProcessSignal::CancelQuery => format!("KILL QUERY {id}"),
        ProcessSignal::Terminate => format!("KILL {id}"),
    }
}

impl MySqlSession {
    async fn processes(&mut self) -> Result<Vec<ProcessInfo>, DriverError> {
        let rows = match self.text_query(&processes_sql(true), &[]).await {
            // No access to performance_schema: list without program names.
            Err(DriverError::Server { .. }) => self.text_query(&processes_sql(false), &[]).await?,
            other => other?,
        };
        rows.iter()
            .map(|row| ProcessInfo::from_text_row(row))
            .collect()
    }

    async fn signal(&mut self, id: ProcessId, signal: ProcessSignal) -> Result<(), DriverError> {
        match self.control(&signal_sql(id, signal), &[]).await {
            Ok(_) => Ok(()),
            Err(DriverError::Server { message, .. }) if message.contains(NO_SUCH_THREAD) => {
                Err(unknown_process(id))
            }
            Err(error) => Err(error),
        }
    }

    async fn connection_id(&mut self) -> Result<ProcessId, DriverError> {
        let rows = self.text_query("SELECT CONNECTION_ID()", &[]).await?;
        rows.first()
            .and_then(|row| row.first().cloned().flatten())
            .and_then(|id| id.parse().ok())
            .ok_or_else(|| DriverError::Protocol("unexpected connection id".to_owned()))
    }
}

impl Admin for MySqlSession {
    fn own_process(&mut self) -> BoxFuture<'_, Result<ProcessId, DriverError>> {
        Box::pin(self.connection_id())
    }

    fn list_processes(&mut self) -> BoxFuture<'_, Result<Vec<ProcessInfo>, DriverError>> {
        Box::pin(self.processes())
    }

    fn signal_process(
        &mut self,
        id: ProcessId,
        signal: ProcessSignal,
    ) -> BoxFuture<'_, Result<(), DriverError>> {
        Box::pin(self.signal(id, signal))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_listing_joins_program_names_only_when_asked() {
        let with = processes_sql(true);
        assert!(with.contains("a.ATTR_VALUE") && with.contains("session_connect_attrs"));
        let without = processes_sql(false);
        assert!(without.contains(", NULL, ") && !without.contains("session_connect_attrs"));
        assert!(!without.contains('{'));
    }

    #[test]
    fn signals_are_kill_statements() {
        assert_eq!(signal_sql(9, ProcessSignal::CancelQuery), "KILL QUERY 9");
        assert_eq!(signal_sql(9, ProcessSignal::Terminate), "KILL 9");
    }
}
