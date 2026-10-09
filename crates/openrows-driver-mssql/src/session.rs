// SPDX-License-Identifier: Apache-2.0
//! A SQL Server connection as a driver [`Session`]: a statement (or a whole
//! T-SQL batch) per call, as a SQL batch or, with bound values, through
//! `sp_executesql`; every result set streams into the sink, messages and
//! `PRINT` output become notices, statement row counts become
//! `(n rows affected)` notices, and cancel sends the attention signal.
use std::sync::Arc;

use openrows_driver_api::{
    BatchPolicy, BoxFuture, CancelHandle, Catalog, Connection, DriverError, Editor, ExecSummary,
    ResultSink, ServerSummary, Session,
};

use crate::error::{cancelled, server_error};
use crate::reply::{Env, Ignore, Outcome, Rows, read_reply};
use crate::rows::{ArrowRows, TextRows};
use crate::tds::packet::PacketType;
use crate::tds::request::{execute_sql, sql_batch};
use crate::tds::wire::{SharedOutbound, Wire};

/// One SQL Server connection, owned by a single session actor (ADR-0007).
pub struct SqlServerSession {
    wire: Wire,
    pub(crate) env: Env,
    pub(crate) summary: ServerSummary,
    cancel: Arc<Attention>,
    policy: BatchPolicy,
}

impl SqlServerSession {
    pub(crate) fn new(wire: Wire, env: Env, summary: ServerSummary) -> Self {
        let cancel = Arc::new(Attention(Arc::clone(&wire.outbound)));
        Self {
            wire,
            env,
            summary,
            cancel,
            policy: BatchPolicy::default(),
        }
    }

    /// Send `sql` (with `params`, through `sp_executesql`) and read the
    /// reply into `rows`; server errors and a cancel become errors.
    pub(crate) async fn request(
        &mut self,
        sql: &str,
        params: &[Option<String>],
        rows: &mut dyn Rows,
    ) -> Result<Outcome, DriverError> {
        let (kind, payload) = if params.is_empty() {
            (PacketType::SqlBatch, sql_batch(sql, self.env.transaction))
        } else {
            let context = (self.env.transaction, self.env.collation);
            (PacketType::Rpc, execute_sql(sql, params, context))
        };
        self.wire.send(kind, &payload).await?;
        let outcome = read_reply(&mut self.wire, &mut self.env, rows).await?;
        if let Some(size) = outcome.packet_size {
            self.wire.set_packet_size(size).await;
        }
        if !self.env.database.is_empty() && self.env.database != self.summary.database {
            self.summary.database.clone_from(&self.env.database);
        }
        if outcome.cancelled {
            return Err(cancelled());
        }
        match &outcome.error {
            Some(error) => Err(server_error(error)),
            None => Ok(outcome),
        }
    }

    async fn run(
        &mut self,
        sql: &str,
        params: &[Option<String>],
        sink: &mut dyn ResultSink,
    ) -> Result<ExecSummary, DriverError> {
        let mut rows = ArrowRows::new(sink, self.policy);
        let outcome = self.request(sql, params, &mut rows).await?;
        let mut summary = std::mem::take(&mut rows.summary);
        let count = if rows.had_rows {
            summary.rows
        } else {
            outcome.counts.iter().map(|count| count.rows).sum()
        };
        summary.command_tag = tag(sql, count);
        Ok(summary)
    }

    /// Run internal SQL and collect its rows as text.
    pub(crate) async fn text_query(
        &mut self,
        sql: &str,
        params: &[Option<String>],
    ) -> Result<Vec<Vec<Option<String>>>, DriverError> {
        let mut rows = TextRows::default();
        self.request(sql, params, &mut rows).await?;
        Ok(rows.rows)
    }

    /// Run a statement whose rows do not matter; returns its outcome.
    pub(crate) async fn control(
        &mut self,
        sql: &str,
        params: &[Option<String>],
    ) -> Result<Outcome, DriverError> {
        self.request(sql, params, &mut Ignore).await
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
        "SELECT" | "WITH" | "INSERT" | "UPDATE" | "DELETE" | "MERGE" | "EXEC" | "EXECUTE"
    );
    if counted {
        format!("{verb} {count}")
    } else {
        verb
    }
}

impl Session for SqlServerSession {
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
        self.env.transaction != 0
    }

    fn close(mut self: Box<Self>) -> BoxFuture<'static, Result<(), DriverError>> {
        Box::pin(async move { self.wire.shutdown().await })
    }
}

impl Connection for SqlServerSession {
    fn server(&self) -> &ServerSummary {
        &self.summary
    }

    fn catalog(&mut self) -> Option<&mut dyn Catalog> {
        Some(self)
    }

    fn editor(&mut self) -> Option<&mut dyn Editor> {
        Some(self)
    }

    fn admin(&mut self) -> Option<&mut dyn openrows_driver_api::Admin> {
        Some(self)
    }
}

/// Cancels the running request with the TDS attention signal on the same
/// connection; the session reads the acknowledgement and stays usable.
pub(crate) struct Attention(SharedOutbound);

impl CancelHandle for Attention {
    fn cancel(&self) -> BoxFuture<'static, Result<(), DriverError>> {
        let outbound = Arc::clone(&self.0);
        Box::pin(async move { outbound.lock().await.attention().await })
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
