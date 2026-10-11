// SPDX-License-Identifier: Apache-2.0
//! The process list (FR-ADMIN-01): client backends from `pg_stat_activity`,
//! `pg_cancel_backend` and `pg_terminate_backend`. Runs through the
//! simple-query protocol on whatever connection offers the facet (the app's
//! metadata connection).
use openrows_driver_api::admin::unknown_process;
use openrows_driver_api::{Admin, BoxFuture, DriverError, ProcessId, ProcessInfo, ProcessSignal};

use crate::session::PgSession;

/// Client backends in [`ProcessInfo::from_text_row`] order, the longest
/// running first. The duration of an active backend counts from its query
/// start, of any other from its last state change.
pub(crate) const ACTIVITY_AGE: &str = "GREATEST(0, (extract(epoch FROM clock_timestamp() - COALESCE(CASE WHEN state = 'active' THEN query_start ELSE state_change END, backend_start)) * 1000)::bigint)";
pub(crate) const ACTIVITY_WAIT: &str =
    "CASE WHEN wait_event IS NULL THEN NULL ELSE wait_event_type || ': ' || wait_event END";

fn processes_sql() -> String {
    format!(
        "\
SELECT pid, usename, datname, \
    CASE WHEN client_addr IS NULL THEN NULL \
        ELSE host(client_addr) || COALESCE(':' || client_port, '') END, \
    application_name, state, \
    {ACTIVITY_WAIT}, query_start::text, {ACTIVITY_AGE}, \
    query, pid = pg_backend_pid() \
FROM pg_stat_activity \
WHERE backend_type = 'client backend' \
ORDER BY 9 DESC, pid"
    )
}

/// The statement signalling `id`, or `None` when no backend can have it
/// (pids are positive `int4`).
fn signal_sql(id: ProcessId, signal: ProcessSignal) -> Option<String> {
    let pid = i32::try_from(id).ok().filter(|pid| *pid > 0)?;
    let function = match signal {
        ProcessSignal::CancelQuery => "pg_cancel_backend",
        ProcessSignal::Terminate => "pg_terminate_backend",
    };
    Some(format!("SELECT {function}({pid})"))
}

/// The single text value of a one-row, one-column result.
fn single(rows: &[Vec<Option<String>>]) -> Option<&str> {
    match rows {
        [row] => row.first()?.as_deref(),
        _ => None,
    }
}

impl PgSession {
    async fn signal(&mut self, id: ProcessId, signal: ProcessSignal) -> Result<(), DriverError> {
        let sql = signal_sql(id, signal).ok_or_else(|| unknown_process(id))?;
        let rows = self.text_query(&sql).await?;
        match single(&rows) {
            Some("t") => Ok(()),
            Some("f") => Err(unknown_process(id)),
            _ => Err(DriverError::Protocol(format!("unexpected reply to {sql}"))),
        }
    }

    async fn backend_pid(&mut self) -> Result<ProcessId, DriverError> {
        let rows = self.text_query("SELECT pg_backend_pid()").await?;
        single(&rows)
            .and_then(|pid| pid.parse().ok())
            .ok_or_else(|| DriverError::Protocol("unexpected backend pid".to_owned()))
    }
}

impl Admin for PgSession {
    fn own_process(&mut self) -> BoxFuture<'_, Result<ProcessId, DriverError>> {
        Box::pin(self.backend_pid())
    }

    fn list_processes(&mut self) -> BoxFuture<'_, Result<Vec<ProcessInfo>, DriverError>> {
        Box::pin(async move {
            let rows = self.text_query(&processes_sql()).await?;
            rows.iter()
                .map(|row| ProcessInfo::from_text_row(row))
                .collect()
        })
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
    fn signals_name_the_backend_function_and_reject_impossible_pids() {
        assert_eq!(
            signal_sql(42, ProcessSignal::CancelQuery).as_deref(),
            Some("SELECT pg_cancel_backend(42)")
        );
        assert_eq!(
            signal_sql(42, ProcessSignal::Terminate).as_deref(),
            Some("SELECT pg_terminate_backend(42)")
        );
        assert_eq!(signal_sql(0, ProcessSignal::Terminate), None);
        assert_eq!(signal_sql(1 << 40, ProcessSignal::CancelQuery), None);
    }

    #[test]
    fn single_values_need_exactly_one_row() {
        let one = vec![vec![Some("t".to_owned())]];
        assert_eq!(single(&one), Some("t"));
        assert_eq!(single(&[]), None);
        assert_eq!(single(&[one[0].clone(), one[0].clone()]), None);
    }
}
