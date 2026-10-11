// SPDX-License-Identifier: Apache-2.0
//! Native calls run synchronously on the existing dedicated host connection thread.
use super::{error, stream};
use openrows_driver_api::{
    BoxFuture, CancelHandle, Connection, DriverError, ExecSummary, FileSource, ResultSink,
    ServerSummary, Session, TlsStatus,
};
use std::sync::Arc;

pub(super) struct DuckdbSession {
    native: duckdb::Connection,
    server: ServerSummary,
    cancel: Arc<Interrupt>,
}
struct Interrupt(Arc<duckdb::InterruptHandle>);
impl CancelHandle for Interrupt {
    fn cancel(&self) -> BoxFuture<'static, Result<(), DriverError>> {
        let handle = self.0.clone();
        Box::pin(async move {
            handle.interrupt();
            Ok(())
        })
    }
}
impl DuckdbSession {
    pub(super) fn new(
        native: duckdb::Connection,
        source: &FileSource,
    ) -> Result<Self, DriverError> {
        let version = native
            .query_row("SELECT version()", [], |row| row.get::<_, String>(0))
            .map_err(error::map)?;
        let flavour = match source {
            FileSource::Memory { .. } => Some(
                "in-memory; closing or losing the driver process permanently loses all data".into(),
            ),
            FileSource::Disk { read_only, .. } => read_only.then(|| "read-only".into()),
        };
        let cancel = Arc::new(Interrupt(native.interrupt_handle()));
        Ok(Self {
            native,
            cancel,
            server: ServerSummary {
                product: "DuckDB".into(),
                version,
                flavour,
                database: "main".into(),
                user: String::new(),
                tls: TlsStatus::LocalSocket,
            },
        })
    }
    pub(super) fn run(
        &mut self,
        sql: &str,
        params: &[Option<String>],
        sink: &mut dyn ResultSink,
    ) -> Result<ExecSummary, DriverError> {
        if openrows_sql::duckdb::has_attachment(sql) {
            return Err(DriverError::Unsupported(
                "DuckDB ATTACH requires attached-file reservations, which are not available yet"
                    .into(),
            ));
        }
        let statements = openrows_sql::split(sql, openrows_sql::Dialect::Duckdb);
        if !params.is_empty() && statements.len() != 1 {
            return Err(DriverError::Unsupported(
                "DuckDB bound execution requires one statement".into(),
            ));
        }
        let mut summary = ExecSummary::default();
        for statement in statements {
            let (result, stopped) = stream::run(&self.native, statement.text(sql), params, sink)?;
            summary.rows += result.rows;
            summary.batches += result.batches;
            summary.command_tag = result.command_tag;
            summary.first_batch_latency =
                summary.first_batch_latency.or(result.first_batch_latency);
            if stopped {
                break;
            }
        }
        Ok(summary)
    }
}
impl Session for DuckdbSession {
    fn execute<'a>(
        &'a mut self,
        sql: &'a str,
        sink: &'a mut dyn ResultSink,
    ) -> BoxFuture<'a, Result<ExecSummary, DriverError>> {
        self.execute_bound(sql, &[], sink)
    }
    fn execute_bound<'a>(
        &'a mut self,
        sql: &'a str,
        params: &'a [Option<String>],
        sink: &'a mut dyn ResultSink,
    ) -> BoxFuture<'a, Result<ExecSummary, DriverError>> {
        Box::pin(async move { self.run(sql, params, sink) })
    }
    fn cancel_handle(&self) -> Option<Arc<dyn CancelHandle>> {
        Some(self.cancel.clone())
    }
    fn ping(&mut self) -> BoxFuture<'_, Result<(), DriverError>> {
        Box::pin(async move { self.native.execute_batch("SELECT 1").map_err(error::map) })
    }
    fn in_transaction(&self) -> bool {
        !self.native.is_autocommit()
    }
    fn close(self: Box<Self>) -> BoxFuture<'static, Result<(), DriverError>> {
        Box::pin(async move {
            drop(self);
            Ok(())
        })
    }
}
impl Connection for DuckdbSession {
    fn server(&self) -> &ServerSummary {
        &self.server
    }
}
