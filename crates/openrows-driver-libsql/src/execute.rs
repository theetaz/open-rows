// SPDX-License-Identifier: Apache-2.0
//! Native bindings, plain sequences and conditional atomic script batches.
use crate::{
    LibsqlSession, error,
    protocol::{Batch, NamedArg, Request, Response, Statement, Value},
};
use openrows_driver_api::{DiscardSink, DriverError, ExecSummary, ResultSink, Session};
use openrows_sql::select::BoundSql;
impl LibsqlSession {
    /// Execute native positional and named Hrana bindings without interpolating SQL.
    /// # Errors
    /// Server, protocol, network, timeout and cancellation errors.
    pub async fn execute_values(
        &mut self,
        sql: &str,
        args: Vec<Value>,
        named_args: Vec<NamedArg>,
        sink: &mut dyn ResultSink,
    ) -> Result<ExecSummary, DriverError> {
        let stmt = Statement {
            sql: sql.into(),
            args,
            named_args,
            want_rows: true,
        };
        let (responses, mut summary) = self
            .request(&[Request::Execute { stmt }, Request::GetAutocommit], sink)
            .await?;
        let count = match responses.first() {
            Some(Response::Execute { result }) => {
                if summary.batches > 0 {
                    summary.rows
                } else {
                    result.affected_row_count
                }
            }
            _ => return Err(error::protocol()),
        };
        summary.command_tag = tag(sql, count);
        Ok(summary)
    }

    /// Run a plain SQL script, ignoring rows; the server stops on its first failure.
    /// # Errors
    /// Server errors leave earlier writes intact unless the script owns a transaction.
    pub async fn sequence(&mut self, sql: &str) -> Result<(), DriverError> {
        self.request(
            &[
                Request::Sequence { sql: sql.into() },
                Request::GetAutocommit,
            ],
            &mut DiscardSink,
        )
        .await
        .map(drop)
    }

    /// Run bound statements with conditional commit/rollback in one immediate transaction.
    /// # Errors
    /// Refused inside an existing transaction; the first failing step is returned.
    pub async fn atomic_batch(
        &mut self,
        statements: &[BoundSql],
        sink: &mut dyn ResultSink,
    ) -> Result<ExecSummary, DriverError> {
        if self.in_transaction() {
            return Err(DriverError::Unsupported(
                "an atomic batch cannot nest inside a transaction".into(),
            ));
        }
        let batch = Batch::atomic(statements);
        let requests = [Request::Batch { batch }, Request::GetAutocommit];
        let (responses, summary) = self.request(&requests, sink).await?;
        let Some(Response::Batch { result }) = responses.into_iter().next() else {
            return Err(error::protocol());
        };
        if result.step_results.len() != statements.len() + 3
            || result.step_errors.len() != statements.len() + 3
        {
            return Err(error::protocol());
        }
        if let Some(error) = result.step_errors.into_iter().flatten().next() {
            return Err(error::server(&error, self.token()));
        }
        Ok(summary)
    }
}
fn tag(sql: &str, count: u64) -> String {
    let verb = sql
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    if matches!(
        verb.as_str(),
        "SELECT" | "WITH" | "VALUES" | "INSERT" | "UPDATE" | "DELETE" | "REPLACE"
    ) {
        format!("{verb} {count}")
    } else {
        verb
    }
}
