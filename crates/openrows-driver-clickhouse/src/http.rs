// SPDX-License-Identifier: Apache-2.0
//! One HTTP exchange on an `OpenRows` route, with bounded control responses.
use crate::{
    error, request,
    transport::{Target, within},
};
use bytes::Bytes;
use http_body_util::BodyExt;
use hyper::{Response, body::Incoming, client::conn::http1};
use hyper_util::rt::TokioIo;
use openrows_driver_api::{ConnectRequest, ConnectStage, ConnectTrace, DriverError, TlsStatus};
use openrows_net::tls::TlsClient;
use std::{io, sync::Arc};

/// Dropping a response closes its HTTP connection and any unread streaming body.
pub(crate) struct Reply {
    pub(crate) response: Response<Incoming>,
    _connection: ConnectionTask,
}
struct ConnectionTask(tokio::task::JoinHandle<()>);
impl Drop for ConnectionTask {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[derive(Clone)]
pub(crate) struct Client {
    pub(crate) config: Arc<ConnectRequest>,
    target: Target,
    tls: Option<TlsClient>,
}
impl Client {
    pub(crate) fn new(config: ConnectRequest) -> Result<Self, DriverError> {
        let target = Target::of(&config.endpoint, config.via.as_ref())?;
        let tls = if config.ssl_mode.requires_tls() {
            Some(
                TlsClient::new(&config.tls.params(config.ssl_mode))
                    .map_err(|e| DriverError::Tls(e.to_string()))?,
            )
        } else {
            None
        };
        Ok(Self {
            config: Arc::new(config),
            target,
            tls,
        })
    }

    pub(crate) async fn send(
        &self,
        sql: &str,
        params: &[Option<String>],
        identity: (&str, Option<&str>),
    ) -> Result<Reply, DriverError> {
        let deadline = tokio::time::Instant::now() + self.config.timeouts.tcp;
        loop {
            let result = self
                .send_traced(sql, params, identity, &mut ConnectTrace::default())
                .await;
            match result {
                // A dropped read response can outlive its client briefly. Code
                // 373 guarantees this request never executed, so retrying it
                // is safe, including for writes. No other failure is replayed.
                Err(DriverError::Server { ref code, .. })
                    if code == "55P03"
                        && identity.1.is_some()
                        && tokio::time::Instant::now() < deadline => {}
                other => return other.map(|(reply, _)| reply),
            }
        }
    }

    pub(crate) async fn send_traced(
        &self,
        sql: &str,
        params: &[Option<String>],
        identity: (&str, Option<&str>),
        trace: &mut ConnectTrace,
    ) -> Result<(Reply, TlsStatus), DriverError> {
        let request = request::build(&self.config, sql, params, identity)?;
        let mut stream = self.target.open(&self.config.timeouts, trace).await?;
        let mut status = TlsStatus::Disabled;
        if let Some(tls) = &self.tls {
            let openrows_driver_api::Endpoint::Server { host, .. } = &self.config.endpoint else {
                return Err(DriverError::Unsupported(
                    "ClickHouse file endpoint".to_owned(),
                ));
            };
            let handshake = async {
                tls.handshake(stream, host)
                    .await
                    .map_err(|e| DriverError::Tls(e.to_string()))
            };
            let (secured, details) = trace
                .stage(
                    ConnectStage::Tls,
                    within(self.config.timeouts.tls, "TLS handshake", handshake),
                )
                .await?;
            status = TlsStatus::Encrypted(details);
            stream = Box::new(secured);
        }
        let (mut sender, connection) = http1::handshake(TokioIo::new(stream))
            .await
            .map_err(network)?;
        let connection = ConnectionTask(tokio::spawn(async move {
            let _ = connection.await;
        }));
        let response = sender.send_request(request).await.map_err(network)?;
        let mut reply = Reply {
            response,
            _connection: connection,
        };
        if !reply.response.status().is_success()
            || reply
                .response
                .headers()
                .contains_key("x-clickhouse-exception-code")
        {
            let code = reply
                .response
                .headers()
                .get("x-clickhouse-exception-code")
                .and_then(|h| h.to_str().ok())
                .map(str::to_owned);
            let body = read_limited(reply.response.body_mut(), 65_536).await?;
            return Err(error::server(
                code.as_deref(),
                &String::from_utf8_lossy(&body),
                &self.config,
            ));
        }
        Ok((reply, status))
    }

    pub(crate) async fn control(
        &self,
        sql: &str,
        params: &[Option<String>],
    ) -> Result<Bytes, DriverError> {
        let id = uuid::Uuid::now_v7().to_string();
        let mut reply = self.send(sql, params, (&id, None)).await?;
        read_limited(reply.response.body_mut(), 1_048_576).await
    }
}

pub(crate) async fn read_limited(body: &mut Incoming, limit: usize) -> Result<Bytes, DriverError> {
    let mut bytes = Vec::new();
    while let Some(frame) = body.frame().await {
        if let Ok(data) = frame.map_err(network)?.into_data() {
            if bytes.len().saturating_add(data.len()) > limit {
                return Err(DriverError::Protocol(
                    "ClickHouse control response exceeds its size limit".to_owned(),
                ));
            }
            bytes.extend_from_slice(&data);
        }
    }
    Ok(Bytes::from(bytes))
}

fn network(error: hyper::Error) -> DriverError {
    DriverError::Network(io::Error::other(error))
}
