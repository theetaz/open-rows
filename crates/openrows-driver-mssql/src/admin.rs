// SPDX-License-Identifier: Apache-2.0
//! The process list (FR-ADMIN-01) from `sys.dm_exec_sessions`, `requests`
//! and `connections`, and `KILL`. Seeing other sessions needs `VIEW SERVER
//! STATE` (`VIEW DATABASE STATE` on Azure SQL Database); without it the
//! list holds this session only. SQL Server cannot cancel another
//! session's statement, only end the session.
use openrows_driver_api::admin::unknown_process;
use openrows_driver_api::{Admin, BoxFuture, DriverError, ProcessId, ProcessInfo, ProcessSignal};

use crate::session::SqlServerSession;

const PROCESSES_SQL: &str = include_str!("catalog/sql/processes.sql");

/// Whether a session with the bound id exists.
const SESSION_SQL: &str = "SELECT 1 FROM sys.dm_exec_sessions WHERE session_id = @P1";

/// `Process ID n is not an active process ID.`
const NOT_ACTIVE: &str = "(SQL Server error 6106";

impl SqlServerSession {
    async fn processes(&mut self) -> Result<Vec<ProcessInfo>, DriverError> {
        let rows = self.text_query(PROCESSES_SQL, &[]).await?;
        rows.iter()
            .map(|row| ProcessInfo::from_text_row(row))
            .collect()
    }

    async fn signal(&mut self, id: ProcessId, signal: ProcessSignal) -> Result<(), DriverError> {
        if signal == ProcessSignal::CancelQuery {
            let found = self
                .text_query(SESSION_SQL, &[Some(id.to_string())])
                .await?;
            if found.is_empty() {
                return Err(unknown_process(id));
            }
            return Err(DriverError::Unsupported(
                "cancelling another session's statement (SQL Server can only end the session)"
                    .to_owned(),
            ));
        }
        // Session ids are integers, so no quoting applies.
        match self.control(&format!("KILL {id}"), &[]).await {
            Ok(_) => Ok(()),
            Err(DriverError::Server { message, .. }) if message.contains(NOT_ACTIVE) => {
                Err(unknown_process(id))
            }
            Err(error) => Err(error),
        }
    }

    async fn session_id(&mut self) -> Result<ProcessId, DriverError> {
        let rows = self.text_query("SELECT @@SPID", &[]).await?;
        rows.first()
            .and_then(|row| row.first().cloned().flatten())
            .and_then(|id| id.parse().ok())
            .ok_or_else(|| DriverError::Protocol("unexpected session id".to_owned()))
    }
}

impl Admin for SqlServerSession {
    fn own_process(&mut self) -> BoxFuture<'_, Result<ProcessId, DriverError>> {
        Box::pin(self.session_id())
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
