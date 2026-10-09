// SPDX-License-Identifier: Apache-2.0
//! Query-list tokens map numeric UI ids to `ClickHouse`'s string query ids.
use crate::{catalog::text, session::ClickHouseSession};
use openrows_driver_api::{
    Admin, BoxFuture, DriverError, ProcessId, ProcessInfo, ProcessSignal, admin::unknown_process,
};
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(1);
impl Admin for ClickHouseSession {
    fn own_process(&mut self) -> BoxFuture<'_, Result<ProcessId, DriverError>> {
        Box::pin(async { Ok(0) })
    }
    fn list_processes(&mut self) -> BoxFuture<'_, Result<Vec<ProcessInfo>, DriverError>> {
        Box::pin(async move {
            let rows = self
                .text_query(include_str!("catalog/sql/processes.sql"), &[])
                .await?;
            let mut processes = Vec::with_capacity(rows.len());
            for row in rows {
                let query_id = text(&row, 0);
                let current = query_id.starts_with(&self.id);
                let id = token(current);
                self.processes.insert(id, query_id);
                processes.push(ProcessInfo {
                    id,
                    user: Some(text(&row, 1)),
                    database: Some(text(&row, 2)),
                    client: Some(text(&row, 3)),
                    query: Some(text(&row, 4)),
                    duration_ms: text(&row, 5).parse().ok(),
                    application: Some(text(&row, 6)),
                    state: Some("active".to_owned()),
                    current,
                    ..ProcessInfo::default()
                });
            }
            // Tokens only apply to this listing; an old token cannot target a later query.
            let ids: std::collections::HashSet<_> = processes.iter().map(|p| p.id).collect();
            self.processes.retain(|id, _| ids.contains(id));
            Ok(processes)
        })
    }
    fn signal_process(
        &mut self,
        id: ProcessId,
        signal: ProcessSignal,
    ) -> BoxFuture<'_, Result<(), DriverError>> {
        Box::pin(async move {
            if signal == ProcessSignal::Terminate {
                return Err(DriverError::Unsupported(
                    "ClickHouse has queries, not persistent server processes; use Cancel query"
                        .to_owned(),
                ));
            }
            let query = self
                .processes
                .get(&id)
                .cloned()
                .ok_or_else(|| unknown_process(id))?;
            if id == 0 {
                return Err(DriverError::Unsupported(
                    "cancelling the process-list query itself".to_owned(),
                ));
            }
            let response = self
                .client
                .control(
                    "KILL QUERY WHERE query_id = {p1:String} SYNC",
                    &[Some(query)],
                )
                .await?;
            if response.is_empty() {
                return Err(unknown_process(id));
            }
            Ok(())
        })
    }
}

fn token(current: bool) -> ProcessId {
    if current {
        0
    } else {
        NEXT.fetch_add(1, Ordering::Relaxed)
    }
}
