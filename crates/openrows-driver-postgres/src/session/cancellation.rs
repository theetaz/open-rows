// SPDX-License-Identifier: Apache-2.0
//! Apply the cancel fence to every session operation that sends SQL.
use super::PgSession;
use crate::cancel::StatementGuard;
use openrows_driver_api::{DriverError, ExecSummary, ResultSink};

impl PgSession {
    pub(super) async fn run_guarded(
        &mut self,
        sql: &str,
        params: &[Option<String>],
        sink: &mut dyn ResultSink,
    ) -> Result<ExecSummary, DriverError> {
        let statement = self.begin_statement().await?;
        let result = self.run(sql, params, sink).await;
        self.finish_statement(statement, result).await
    }

    /// Start a statement generation, excluding any earlier cancel still in flight.
    pub(crate) async fn begin_statement(&mut self) -> Result<Option<StatementGuard>, DriverError> {
        match &self.cancel {
            Some(cancel) => cancel.begin().await.map(Some),
            None => Ok(None),
        }
    }

    /// Preserve the result only when no cancellation can escape into later SQL.
    pub(crate) async fn finish_statement<T>(
        &mut self,
        statement: Option<StatementGuard>,
        result: Result<T, DriverError>,
    ) -> Result<T, DriverError> {
        if let Some(statement) = statement
            && let Err(error) = statement.finish().await
        {
            self.wire.invalidate();
            return Err(error);
        }
        result
    }
}
