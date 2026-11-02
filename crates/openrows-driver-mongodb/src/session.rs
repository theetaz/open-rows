// SPDX-License-Identifier: Apache-2.0
use crate::{cancel::Cancel, command, error, proxy::Proxy};
use mongodb::{Client, bson::Document};
use openrows_driver_api::{
    BoxFuture, CancelHandle, Connection, DriverError, ExecSummary, ResultSink, ServerSummary,
    Session,
};
use std::sync::Arc;

/// An actor-owned `MongoDB` database session and its network adapter.
pub struct MongoSession {
    pub(crate) client: Client,
    pub(crate) summary: ServerSummary,
    cancel: Arc<Cancel>,
    _proxy: Proxy,
}
impl MongoSession {
    pub(crate) fn new(
        client: Client,
        summary: ServerSummary,
        proxy: Proxy,
        control: Arc<crate::cancel::State>,
    ) -> Self {
        Self {
            cancel: Cancel::new(client.clone(), control),
            client,
            summary,
            _proxy: proxy,
        }
    }
    pub(crate) fn database(&self) -> mongodb::Database {
        self.client.database(&self.summary.database)
    }
    async fn execute_query(
        &self,
        source: &str,
        sink: &mut dyn ResultSink,
    ) -> Result<ExecSummary, DriverError> {
        let query = openrows_sql::mql::parse(source)
            .map_err(|error| DriverError::Unsupported(error.to_string()))?;
        let token = uuid::Uuid::now_v7().to_string();
        let mut command = command::command(&query)?;
        command.insert("comment", &token);
        let mut cancelled = self.cancel.signal.subscribe();
        let generation = *cancelled.borrow();
        self.cancel
            .set(Some(token.clone()), query.method == "createIndex");
        let mut outcome = tokio::select! {
            result = self.run(command, (&token, query.document_view, query.method == "countDocuments"), sink) => result,
            _ = cancelled.changed() => Err(cancelled_error(query.classification().is_write())),
        };
        if outcome.is_err() && *self.cancel.signal.borrow() != generation {
            outcome = Err(cancelled_error(query.classification().is_write()));
        }
        self.cancel.set(None, false);
        if query.classification().is_write()
            && outcome.as_ref().is_err_and(|error| {
                error.is_connection_lost() || matches!(error, DriverError::Protocol(_))
            })
        {
            return Err(DriverError::Server {
                code: "08007".into(),
                message:
                    "MongoDB write outcome is unknown after a lost response; verify before retrying"
                        .into(),
            });
        }
        outcome
    }
    async fn run(
        &self,
        command: Document,
        (token, table, count): (&str, bool, bool),
        sink: &mut dyn ResultSink,
    ) -> Result<ExecSummary, DriverError> {
        if ["find", "aggregate", "listIndexes"]
            .iter()
            .any(|name| command.contains_key(*name))
        {
            return crate::streaming::run(&self.database(), command, (token, table, count), sink)
                .await;
        }
        let reply = self
            .database()
            .run_command(command)
            .await
            .map_err(error::mongo)?;
        command::check_reply(&reply)?;
        let affected = reply
            .get_i32("n")
            .ok()
            .and_then(|n| u64::try_from(n).ok())
            .unwrap_or_default();
        Ok(ExecSummary {
            command_tag: format!("MongoDB {affected}"),
            ..ExecSummary::default()
        })
    }
}
impl Session for MongoSession {
    fn execute<'a>(
        &'a mut self,
        source: &'a str,
        sink: &'a mut dyn ResultSink,
    ) -> BoxFuture<'a, Result<ExecSummary, DriverError>> {
        Box::pin(self.execute_query(source, sink))
    }
    fn cancel_handle(&self) -> Option<Arc<dyn CancelHandle>> {
        Some(self.cancel.clone())
    }
    fn ping(&mut self) -> BoxFuture<'_, Result<(), DriverError>> {
        Box::pin(crate::driver::ping(&self.client))
    }
    fn connection(&mut self) -> Option<&mut dyn Connection> {
        Some(self)
    }
    fn close(self: Box<Self>) -> BoxFuture<'static, Result<(), DriverError>> {
        Box::pin(async move {
            self.client.shutdown().await;
            Ok(())
        })
    }
}
impl Connection for MongoSession {
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

fn cancelled_error(write: bool) -> DriverError {
    DriverError::Server {
        code: if write { "08007" } else { "57014" }.into(),
        message: if write {
            "MongoDB write cancelled: its outcome is unknown; verify before retrying"
        } else {
            "MongoDB query cancelled"
        }
        .into(),
    }
}
