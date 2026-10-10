// SPDX-License-Identifier: Apache-2.0
//! Serial baton requests preserve transaction state across HTTP connections.
use crate::{
    ResultLimits,
    cancel::Abort,
    error,
    http::Http,
    protocol::{Pipeline, Reply, Request, Response, Statement, StreamResult, Value},
};
use openrows_driver_api::{
    BoxFuture, CancelHandle, Connection, DiscardSink, DriverError, ExecSummary, ResultSink,
    ServerSummary, Session,
};
use openrows_net::SecretString;
use openrows_sql::{Dialect, select::BoundSql};
use std::sync::Arc;

/// One remote Hrana stream, exclusively owned by its session actor.
pub struct LibsqlSession {
    pub(crate) http: Http,
    pub(crate) summary: ServerSummary,
    baton: Option<SecretString>,
    cancel: Arc<Abort>,
    limits: ResultLimits,
    transaction: bool,
    usable: bool,
}

impl LibsqlSession {
    pub(crate) fn new(http: Http, summary: ServerSummary, limits: ResultLimits) -> Self {
        Self {
            http,
            summary,
            baton: None,
            cancel: Arc::new(Abort::new()),
            limits,
            transaction: false,
            usable: true,
        }
    }

    pub(crate) fn token(&self) -> Option<&str> {
        self.http.token.as_ref().map(SecretString::expose)
    }

    pub(crate) fn usable(&self) -> bool {
        self.usable
    }

    pub(crate) async fn request(
        &mut self,
        requests: &[Request],
        sink: &mut dyn ResultSink,
    ) -> Result<(Vec<Response>, ExecSummary), DriverError> {
        if !self.usable {
            return Err(DriverError::Protocol(
                "libSQL stream lost; transaction outcome may be unknown; reconnect explicitly"
                    .into(),
            ));
        }
        let transaction_risk = self.transaction || requests.iter().any(opens_transaction);
        let baton = self.baton.clone();
        let pipeline = Pipeline {
            baton: baton.as_ref().map(SecretString::expose),
            requests,
        };
        let mut cancelled = self.cancel.0.subscribe();
        let response = tokio::select! {
            biased;
            _ = cancelled.changed() => Err(error::cancelled()),
            result = tokio::time::timeout(self.limits.timeout, self.http.request(&pipeline, self.limits, sink)) => {
                result.unwrap_or_else(|_| Err(error::lost("libSQL HTTP request timed out; the server may finish the statement")))
            }
        };
        match response {
            Ok((reply, summary)) => {
                let accepted = self.accept(reply, requests);
                if matches!(
                    &accepted,
                    Err(DriverError::Protocol(_) | DriverError::Unsupported(_))
                ) {
                    self.abandon().await;
                    self.usable = false;
                }
                Ok((accepted?, summary))
            }
            Err(error) => {
                self.abandon().await;
                // Stateless cancellation can reopen; a transaction outcome stays unknown.
                self.usable = !transaction_risk
                    && matches!(&error, DriverError::Server { code, .. } if code == "57014");
                if transaction_risk && error.is_connection_lost() {
                    Err(DriverError::Protocol(
                        "libSQL request interrupted; transaction outcome is unknown; reconnect explicitly".into(),
                    ))
                } else {
                    Err(error)
                }
            }
        }
    }

    fn accept(&mut self, reply: Reply, requests: &[Request]) -> Result<Vec<Response>, DriverError> {
        let valid = reply.validate(requests);
        self.baton = reply.baton.map(SecretString::from);
        valid?;
        if let Some(base) = reply.base_url {
            self.http.transport.redirect(&base)?;
        }
        let mut responses = Vec::new();
        let mut failed = None;
        for result in reply.results {
            match result {
                StreamResult::Ok {
                    response: Response::GetAutocommit { is_autocommit },
                } => {
                    self.transaction = !is_autocommit;
                    responses.push(Response::GetAutocommit { is_autocommit });
                }
                StreamResult::Ok { response } => {
                    responses.push(response);
                }
                StreamResult::Error { error } => {
                    failed.get_or_insert_with(|| error::server(&error, self.token()));
                }
            }
        }
        failed.map_or(Ok(responses), Err)
    }

    async fn abandon(&mut self) {
        let was_transaction = self.transaction;
        if self.baton.is_some() {
            let baton = self.baton.take();
            let pipeline = Pipeline {
                baton: baton.as_ref().map(SecretString::expose),
                requests: &[Request::Close],
            };
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(1),
                self.http.request(&pipeline, self.limits, &mut DiscardSink),
            )
            .await;
        }
        // An aborted transaction's outcome is unknown; never open a replacement silently.
        self.usable = !was_transaction;
    }

    pub(crate) async fn control(
        &mut self,
        sql: &str,
        params: &[Option<String>],
    ) -> Result<u64, DriverError> {
        let mut stmt = Statement::text(sql, params);
        stmt.want_rows = false;
        let (responses, _) = self
            .request(
                &[Request::Execute { stmt }, Request::GetAutocommit],
                &mut DiscardSink,
            )
            .await?;
        match responses.into_iter().next() {
            Some(Response::Execute { result }) => Ok(result.affected_row_count),
            _ => Err(error::protocol()),
        }
    }
}

impl Session for LibsqlSession {
    fn execute<'a>(
        &'a mut self,
        sql: &'a str,
        sink: &'a mut dyn ResultSink,
    ) -> BoxFuture<'a, Result<ExecSummary, DriverError>> {
        Box::pin(async move {
            let statements = openrows_sql::split(sql, Dialect::Sqlite);
            if statements.len() > 1 {
                let bound: Vec<_> = statements
                    .iter()
                    .map(|statement| BoundSql {
                        sql: statement.text(sql).into(),
                        params: Vec::new(),
                    })
                    .collect();
                self.atomic_batch(&bound, sink).await
            } else {
                self.execute_values(sql, Vec::new(), Vec::new(), sink).await
            }
        })
    }
    fn execute_bound<'a>(
        &'a mut self,
        sql: &'a str,
        params: &'a [Option<String>],
        sink: &'a mut dyn ResultSink,
    ) -> BoxFuture<'a, Result<ExecSummary, DriverError>> {
        Box::pin(
            self.execute_values(
                sql,
                params
                    .iter()
                    .map(|value| Value::text(value.as_deref()))
                    .collect(),
                Vec::new(),
                sink,
            ),
        )
    }
    fn cancel_handle(&self) -> Option<Arc<dyn CancelHandle>> {
        Some(self.cancel.clone())
    }
    fn in_transaction(&self) -> bool {
        self.transaction
    }
    fn close(mut self: Box<Self>) -> BoxFuture<'static, Result<(), DriverError>> {
        Box::pin(async move {
            if self.baton.is_some() && self.usable {
                self.request(&[Request::Close], &mut DiscardSink).await?;
            }
            Ok(())
        })
    }
}

impl Connection for LibsqlSession {
    fn server(&self) -> &ServerSummary {
        &self.summary
    }
    fn catalog(&mut self) -> Option<&mut dyn openrows_driver_api::Catalog> {
        Some(self)
    }
    fn editor(&mut self) -> Option<&mut dyn openrows_driver_api::Editor> {
        Some(self)
    }
}

fn opens_transaction(request: &Request) -> bool {
    match request {
        Request::Batch { .. } | Request::Sequence { .. } => true,
        Request::Execute { stmt } => {
            openrows_sql::classify(&stmt.sql, Dialect::Sqlite).kind
                == openrows_sql::StatementKind::TxControl
        }
        Request::Close | Request::GetAutocommit => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commented_transaction_control_is_never_reopened_after_abort() {
        for sql in ["/* test */ BEGIN", "-- test\nSAVEPOINT x", "COMMIT"] {
            assert!(opens_transaction(&Request::Execute {
                stmt: Statement::text(sql, &[]),
            }));
        }
        assert!(!opens_transaction(&Request::Execute {
            stmt: Statement::text("SELECT 'BEGIN'", &[]),
        }));
    }
}
