// SPDX-License-Identifier: Apache-2.0
//! Stateful HTTP session and out-of-band query cancellation.
use crate::{
    error,
    http::{Client, read_limited},
    stream,
};
use openrows_driver_api::{
    BoxFuture, CancelHandle, Catalog, Connection, DriverError, ExecSummary, ResultSink,
    ServerSummary, Session,
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::sync::watch;

pub(crate) struct Active {
    id: String,
    stopped: watch::Sender<bool>,
}
pub(crate) struct Cancellation {
    client: Client,
    active: Mutex<Option<Active>>,
}

/// A `ClickHouse` session; metadata and streaming queries share a server session id.
pub struct ClickHouseSession {
    pub(crate) client: Client,
    pub(crate) id: String,
    pub(crate) summary: ServerSummary,
    pub(crate) processes: HashMap<u64, String>,
    cancel: Arc<Cancellation>,
}
impl ClickHouseSession {
    pub(crate) fn new(client: Client, id: String, summary: ServerSummary) -> Self {
        let cancel = Arc::new(Cancellation {
            client: client.clone(),
            active: Mutex::new(None),
        });
        Self {
            client,
            id,
            summary,
            processes: HashMap::new(),
            cancel,
        }
    }

    pub(crate) async fn text_query(
        &self,
        sql: &str,
        params: &[Option<String>],
    ) -> Result<Vec<Vec<serde_json::Value>>, DriverError> {
        let id = format!("{}-{}", self.id, uuid::Uuid::now_v7());
        self.text_query_id(sql, params, &id).await
    }

    async fn text_query_id(
        &self,
        sql: &str,
        params: &[Option<String>],
        id: &str,
    ) -> Result<Vec<Vec<serde_json::Value>>, DriverError> {
        let sql = format!("{sql} FORMAT JSONCompactEachRow");
        let mut reply = self.client.send(&sql, params, (id, Some(&self.id))).await?;
        let bytes = read_limited(reply.response.body_mut(), 4_194_304).await?;
        String::from_utf8_lossy(&bytes)
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| {
                serde_json::from_str(line)
                    .map_err(|_| DriverError::Protocol("invalid ClickHouse catalog row".to_owned()))
            })
            .collect()
    }

    async fn run(
        &mut self,
        sql: &str,
        params: &[Option<String>],
        sink: &mut dyn ResultSink,
    ) -> Result<ExecSummary, DriverError> {
        let id = format!("{}-{}", self.id, uuid::Uuid::now_v7());
        let (stopped, mut cancel) = watch::channel(false);
        *self.cancel.active.lock().map_err(poisoned)? = Some(Active {
            id: id.clone(),
            stopped,
        });
        let outcome = tokio::select! {
            result = self.execute_request(sql, params, (&id, sink)) => result,
            _ = cancel.changed() => Err(error::cancelled()),
        };
        *self.cancel.active.lock().map_err(poisoned)? = None;
        outcome
    }

    async fn execute_request(
        &self,
        sql: &str,
        params: &[Option<String>],
        output: (&str, &mut dyn ResultSink),
    ) -> Result<ExecSummary, DriverError> {
        let (id, sink) = output;
        let sql = sql.trim().trim_end_matches(';').trim_end();
        let class = openrows_sql::classify(sql, openrows_sql::Dialect::ClickHouse);
        let read = class.kind == openrows_sql::StatementKind::Read;
        let select = matches!(crate::query::verb(sql).as_str(), "SELECT" | "WITH");
        let types = if select {
            self.result_types(sql, params, id).await?
        } else {
            Vec::new()
        };
        let query = if select {
            crate::query::arrow_sql(sql, &types)?
        } else {
            sql.to_owned()
        };
        let mut reply = self
            .client
            .send(&query, params, (id, Some(&self.id)))
            .await?;
        let mut summary = if read {
            stream::ingest(reply, types, sink, Arc::clone(&self.client.config)).await?
        } else {
            let count = written_rows(&reply);
            read_limited(reply.response.body_mut(), 65_536).await?;
            ExecSummary {
                command_tag: format!("{} {count}", crate::query::verb(sql)),
                ..ExecSummary::default()
            }
        };
        if read {
            summary.command_tag = format!("{} {}", crate::query::verb(sql), summary.rows);
        }
        Ok(summary)
    }

    async fn result_types(
        &self,
        sql: &str,
        params: &[Option<String>],
        id: &str,
    ) -> Result<Vec<(String, String)>, DriverError> {
        let describe = format!("DESCRIBE TABLE ({sql})");
        self.text_query_id(&describe, params, id)
            .await?
            .into_iter()
            .map(|row| {
                let name = row.first().and_then(serde_json::Value::as_str);
                let kind = row.get(1).and_then(serde_json::Value::as_str);
                match (name, kind) {
                    (Some(name), Some(kind)) => Ok((name.to_owned(), kind.to_owned())),
                    _ => Err(DriverError::Protocol(
                        "invalid ClickHouse result description".to_owned(),
                    )),
                }
            })
            .collect()
    }
}

fn written_rows(reply: &crate::http::Reply) -> u64 {
    reply
        .response
        .headers()
        .get("x-clickhouse-summary")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
        .and_then(|value| value.get("written_rows")?.as_str()?.parse::<u64>().ok())
        .unwrap_or(0)
}

fn poisoned<T>(_: std::sync::PoisonError<T>) -> DriverError {
    DriverError::Protocol("ClickHouse cancel state unavailable".to_owned())
}

impl Session for ClickHouseSession {
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
        Some(self.cancel.clone())
    }
    fn close(self: Box<Self>) -> BoxFuture<'static, Result<(), DriverError>> {
        Box::pin(async { Ok(()) })
    }
}
impl Connection for ClickHouseSession {
    fn server(&self) -> &ServerSummary {
        &self.summary
    }
    fn catalog(&mut self) -> Option<&mut dyn Catalog> {
        Some(self)
    }
    fn admin(&mut self) -> Option<&mut dyn openrows_driver_api::Admin> {
        Some(self)
    }
}
impl CancelHandle for Cancellation {
    fn cancel(&self) -> BoxFuture<'static, Result<(), DriverError>> {
        let client = self.client.clone();
        let active = self.active.lock().map_err(poisoned).map(|state| {
            state
                .as_ref()
                .map(|active| (active.id.clone(), active.stopped.clone()))
        });
        Box::pin(async move {
            if let Some((id, stopped)) = active? {
                let sql = "KILL QUERY WHERE query_id = {p1:String} SYNC";
                let outcome = crate::transport::within(
                    std::time::Duration::from_secs(10),
                    "ClickHouse cancellation",
                    client.control(sql, &[Some(id)]),
                )
                .await;
                let _ = stopped.send(true);
                outcome?;
            }
            Ok(())
        })
    }
}
