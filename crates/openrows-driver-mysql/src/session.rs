// SPDX-License-Identifier: Apache-2.0
//! A `MySQL`/`MariaDB` connection as a driver [`Session`]: one statement per
//! call (text protocol, or a prepared statement for bound parameters),
//! warnings as notices, cancel by `KILL QUERY` on a side connection.
use std::sync::Arc;

use openrows_driver_api::{
    BatchPolicy, BoxFuture, CancelHandle, Catalog, ConnectRequest, ConnectTrace, Connection,
    DriverError, Editor, ExecSummary, NoticeLevel, ResultSink, ServerSummary, Session,
};

use crate::handshake::establish;
use crate::rows::{ArrowRows, Discard, TextRows};
use crate::transport::Target;
use crate::wire::{Outcome, Wire};

/// Most warnings forwarded per statement.
const MAX_WARNINGS: usize = 64;

/// One `MySQL` connection, owned by a single session actor (ADR-0007).
pub struct MySqlSession {
    pub(crate) wire: Wire,
    summary: ServerSummary,
    cancel: Arc<KillQuery>,
    policy: BatchPolicy,
}

impl MySqlSession {
    pub(crate) fn new(wire: Wire, summary: ServerSummary, cancel: KillQuery) -> Self {
        Self {
            wire,
            summary,
            cancel: Arc::new(cancel),
            policy: BatchPolicy::default(),
        }
    }

    async fn run(
        &mut self,
        sql: &str,
        params: &[Option<String>],
        sink: &mut dyn ResultSink,
    ) -> Result<ExecSummary, DriverError> {
        if sql.contains('\0') || params.iter().flatten().any(|value| value.contains('\0')) {
            return Err(DriverError::Unsupported(
                "SQL or values containing NUL characters".to_owned(),
            ));
        }
        let mut rows = ArrowRows::new(sink, self.policy);
        let outcome = if params.is_empty() {
            self.wire.query(sql, &mut rows).await?
        } else {
            self.wire.execute_bound(sql, params, &mut rows).await?
        };
        let mut summary = std::mem::take(&mut rows.summary);
        let count = if rows.had_rows {
            summary.rows
        } else {
            outcome.affected
        };
        summary.command_tag = tag(sql, count);
        drop(rows);
        if outcome.warnings > 0 {
            self.forward_warnings(sink).await?;
        }
        Ok(summary)
    }

    /// `SHOW WARNINGS` as notices: `Note` is informational, the rest warn.
    async fn forward_warnings(&mut self, sink: &mut dyn ResultSink) -> Result<(), DriverError> {
        let mut rows = TextRows::default();
        self.wire.query("SHOW WARNINGS", &mut rows).await?;
        for row in rows.0.iter().take(MAX_WARNINGS) {
            let text = |index: usize| row.get(index).cloned().flatten().unwrap_or_default();
            let level = if text(0) == "Note" {
                NoticeLevel::Info
            } else {
                NoticeLevel::Warning
            };
            sink.notice(
                level,
                &format!("{} (MySQL {} {})", text(2), text(0).to_lowercase(), text(1)),
            );
        }
        Ok(())
    }

    /// Run internal SQL and collect its rows as text.
    pub(crate) async fn text_query(
        &mut self,
        sql: &str,
        params: &[Option<String>],
    ) -> Result<Vec<Vec<Option<String>>>, DriverError> {
        let mut rows = TextRows::default();
        if params.is_empty() {
            self.wire.query(sql, &mut rows).await?;
        } else {
            self.wire.execute_bound(sql, params, &mut rows).await?;
        }
        Ok(rows.0)
    }

    /// Run a statement whose rows do not matter; returns its outcome.
    pub(crate) async fn control(
        &mut self,
        sql: &str,
        params: &[Option<String>],
    ) -> Result<Outcome, DriverError> {
        if params.is_empty() {
            self.wire.query(sql, &mut Discard).await
        } else {
            self.wire.execute_bound(sql, params, &mut Discard).await
        }
    }
}

/// A `PostgreSQL`-style command tag, e.g. `SELECT 3` or `UPDATE 1`.
fn tag(sql: &str, count: u64) -> String {
    let verb = sql
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let counted = matches!(
        verb.as_str(),
        "SELECT"
            | "WITH"
            | "VALUES"
            | "TABLE"
            | "SHOW"
            | "INSERT"
            | "UPDATE"
            | "DELETE"
            | "REPLACE"
    );
    if counted {
        format!("{verb} {count}")
    } else {
        verb
    }
}

impl Session for MySqlSession {
    fn execute<'a>(
        &'a mut self,
        sql: &'a str,
        sink: &'a mut dyn ResultSink,
    ) -> BoxFuture<'a, Result<ExecSummary, DriverError>> {
        Box::pin(self.run(sql, &[], sink))
    }

    fn execute_bound<'a>(
        &'a mut self,
        sql: &'a str,
        params: &'a [Option<String>],
        sink: &'a mut dyn ResultSink,
    ) -> BoxFuture<'a, Result<ExecSummary, DriverError>> {
        Box::pin(self.run(sql, params, sink))
    }

    fn cancel_handle(&self) -> Option<Arc<dyn CancelHandle>> {
        Some(Arc::clone(&self.cancel) as Arc<dyn CancelHandle>)
    }

    fn in_transaction(&self) -> bool {
        self.wire.in_transaction
    }

    fn close(mut self: Box<Self>) -> BoxFuture<'static, Result<(), DriverError>> {
        Box::pin(async move { self.wire.quit().await })
    }
}

impl Connection for MySqlSession {
    fn server(&self) -> &ServerSummary {
        &self.summary
    }

    fn catalog(&mut self) -> Option<&mut dyn Catalog> {
        Some(self)
    }

    fn editor(&mut self) -> Option<&mut dyn Editor> {
        Some(self)
    }
}

/// `KILL QUERY <id>` from a fresh connection with the same login and route.
pub(crate) struct KillQuery {
    pub(crate) request: ConnectRequest,
    pub(crate) target: Target,
    pub(crate) connection_id: u32,
}

impl CancelHandle for KillQuery {
    fn cancel(&self) -> BoxFuture<'static, Result<(), DriverError>> {
        let (request, target, id) = (
            self.request.clone(),
            self.target.clone(),
            self.connection_id,
        );
        Box::pin(async move {
            let established = establish(&request, &target, &mut ConnectTrace::default()).await?;
            let mut wire = Wire {
                packets: established.packets,
                capabilities: established.capabilities,
                in_transaction: false,
            };
            let killed = wire.query(&format!("KILL QUERY {id}"), &mut Discard).await;
            // The side connection is done either way.
            let _ = wire.quit().await;
            killed.map(|_| ())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::tag;

    #[test]
    fn tags_count_rows_or_affected_rows() {
        assert_eq!(tag("select * from t", 3), "SELECT 3");
        assert_eq!(tag("UPDATE t SET a = 1", 2), "UPDATE 2");
        assert_eq!(tag("CREATE TABLE t (a int)", 0), "CREATE");
    }
}
