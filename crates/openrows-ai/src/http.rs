// SPDX-License-Identifier: Apache-2.0
//! The existing Hyper/openrows-net stack; one socket per request, no redirects.
use crate::{AiError, ProviderKind};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{Request, Response, body::Incoming, client::conn::http1, header::HeaderValue};
use hyper_util::rt::TokioIo;
use openrows_net::{
    BoxStream, Dialer, TcpDialer,
    tls::{TlsClient, TlsParams, Verification},
};
use openrows_secrets::SecretString;
use std::sync::Arc;
use tokio::task::JoinHandle;
use url::{Position, Url};

/// Shared route and custom CA/client-certificate options; system trust by default.
pub struct NetworkOptions {
    /// Explicit proxy/SSH route from openrows-net, or direct TCP when absent.
    pub route: Option<Arc<dyn Dialer>>,
    /// Existing openrows-net TLS configuration; full verification is mandatory.
    pub tls: TlsParams,
}
impl Default for NetworkOptions {
    fn default() -> Self {
        Self {
            route: None,
            tls: TlsParams::new(Verification::Full),
        }
    }
}
pub(crate) struct Http {
    pub(crate) url: Url,
    route: Arc<dyn Dialer>,
    tls: Option<TlsClient>,
    pub(crate) key: Option<SecretString>,
    kind: ProviderKind,
}
/// Aborting the connection on drop closes it on cancellation, timeout and error.
pub(crate) struct Exchange {
    pub(crate) response: Response<Incoming>,
    task: JoinHandle<()>,
}
impl Drop for Exchange {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Http {
    pub(crate) fn new(
        url: Url,
        kind: ProviderKind,
        key: Option<SecretString>,
        options: NetworkOptions,
    ) -> Result<Self, AiError> {
        if options.tls.verification != Verification::Full {
            return Err(AiError::InvalidConfig);
        }
        let host = url
            .host_str()
            .ok_or(AiError::InvalidConfig)?
            .trim_matches(['[', ']']);
        let host = if host == "localhost" {
            "127.0.0.1"
        } else {
            host
        };
        let port = url.port_or_known_default().ok_or(AiError::InvalidConfig)?;
        let route = options
            .route
            .unwrap_or_else(|| Arc::new(TcpDialer::new(host.into(), port)));
        let tls = (url.scheme() == "https")
            .then(|| TlsClient::new(&options.tls))
            .transpose()
            .map_err(|_| AiError::Network)?;
        Ok(Self {
            url,
            route,
            tls,
            key,
            kind,
        })
    }
    async fn open(&self) -> Result<BoxStream, AiError> {
        let stream = self.route.dial().await.map_err(|_| AiError::Network)?;
        let Some(tls) = &self.tls else {
            return Ok(stream);
        };
        let host = self
            .url
            .host_str()
            .ok_or(AiError::InvalidConfig)?
            .trim_matches(['[', ']']);
        let (stream, _) = tls
            .handshake(stream, host)
            .await
            .map_err(|_| AiError::Network)?;
        Ok(Box::new(stream))
    }
    pub(crate) async fn request(
        &self,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<Exchange, AiError> {
        let request = self.build(path, body)?;
        let (mut sender, connection) = http1::handshake(TokioIo::new(self.open().await?))
            .await
            .map_err(|_| AiError::Network)?;
        let task = tokio::spawn(async move {
            let _ = connection.await;
        });
        let mut exchange = Pending::new(task);
        let response = sender
            .send_request(request)
            .await
            .map_err(|_| AiError::Network)?;
        let task = exchange.0.take().ok_or(AiError::Network)?;
        Ok(Exchange { response, task })
    }
    fn build(
        &self,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<Request<Full<Bytes>>, AiError> {
        let url = self.url.join(path).map_err(|_| AiError::InvalidConfig)?;
        if url.origin() != self.url.origin() {
            return Err(AiError::InvalidConfig);
        }
        let authority = &url[Position::BeforeHost..Position::AfterPort];
        let target = &url[Position::BeforePath..Position::AfterQuery];
        let method = if body.is_some() { "POST" } else { "GET" };
        let bytes = body
            .map(|value| serde_json::to_vec(&value))
            .transpose()
            .map_err(|_| AiError::InvalidConfig)?
            .unwrap_or_default();
        let mut request = Request::builder()
            .method(method)
            .uri(target)
            .header("host", authority)
            .header("content-type", "application/json")
            .body(Full::new(Bytes::from(bytes)))
            .map_err(|_| AiError::InvalidConfig)?;
        self.authenticate(&mut request)?;
        Ok(request)
    }
    fn authenticate(&self, request: &mut Request<Full<Bytes>>) -> Result<(), AiError> {
        if self.kind == ProviderKind::Anthropic {
            request
                .headers_mut()
                .insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
        }
        let Some(key) = &self.key else {
            return if self.kind.is_local() {
                Ok(())
            } else {
                Err(AiError::Auth)
            };
        };
        let (name, value) = match self.kind {
            ProviderKind::Anthropic => ("x-api-key", key.expose().to_owned()),
            ProviderKind::Gemini => ("x-goog-api-key", key.expose().to_owned()),
            ProviderKind::OpenAi
            | ProviderKind::OpenRouter
            | ProviderKind::Compatible
            | ProviderKind::Ollama
            | ProviderKind::LocalCompatible => {
                ("authorization", format!("Bearer {}", key.expose()))
            }
        };
        let mut header = HeaderValue::from_str(&value).map_err(|_| AiError::Auth)?;
        header.set_sensitive(true);
        request.headers_mut().insert(name, header);
        Ok(())
    }
    pub(crate) fn json(&self, bytes: &[u8]) -> Result<serde_json::Value, AiError> {
        // Metadata and stream frames may echo a key. Refuse the entire frame,
        // including JSON-escaped echoes, before exposing any provider text.
        let value: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|_| AiError::InvalidResponse)?;
        if self
            .key
            .as_ref()
            .is_some_and(|key| contains_secret(&value, key.expose()))
        {
            return Err(AiError::InvalidResponse);
        }
        Ok(value)
    }
}
struct Pending(Option<JoinHandle<()>>);
impl Pending {
    fn new(task: JoinHandle<()>) -> Self {
        Self(Some(task))
    }
}
impl Drop for Pending {
    fn drop(&mut self) {
        if let Some(task) = &self.0 {
            task.abort();
        }
    }
}
fn contains_secret(value: &serde_json::Value, key: &str) -> bool {
    if key.is_empty() {
        return false;
    }
    match value {
        serde_json::Value::String(text) => text.contains(key),
        serde_json::Value::Array(items) => items.iter().any(|item| contains_secret(item, key)),
        serde_json::Value::Object(fields) => fields
            .iter()
            .any(|(name, value)| name.contains(key) || contains_secret(value, key)),
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {
            false
        }
    }
}
pub(crate) fn status(response: &Response<Incoming>) -> Result<(), AiError> {
    let status = response.status().as_u16();
    match status {
        200..=299 => Ok(()),
        401 | 403 => Err(AiError::Auth),
        402 => Err(AiError::Quota),
        429 => Err(AiError::RateLimit {
            retry_after: response
                .headers()
                .get("retry-after")
                .and_then(|header| header.to_str().ok())
                .and_then(|text| text.parse().ok()),
        }),
        404 | 405 | 501 => Err(AiError::Unsupported),
        _ => Err(AiError::Network),
    }
}
pub(crate) async fn read_json(
    http: &Http,
    exchange: &mut Exchange,
) -> Result<serde_json::Value, AiError> {
    check_status(exchange).await?;
    let mut bytes = Vec::new();
    while let Some(frame) = exchange.response.body_mut().frame().await {
        let frame = frame.map_err(|_| AiError::Network)?;
        if let Ok(data) = frame.into_data() {
            if bytes.len().saturating_add(data.len()) > 2 * 1024 * 1024 {
                return Err(AiError::InvalidResponse);
            }
            bytes.extend_from_slice(&data);
        }
    }
    http.json(&bytes)
}

/// Inspect only known error codes; discard provider messages (which can echo keys).
pub(crate) async fn check_status(exchange: &mut Exchange) -> Result<(), AiError> {
    let Err(error) = status(&exchange.response) else {
        return Ok(());
    };
    if !matches!(error, AiError::RateLimit { .. } | AiError::Auth) {
        return Err(error);
    }
    let mut bytes = Vec::new();
    while let Some(frame) = exchange.response.body_mut().frame().await {
        let frame = frame.map_err(|_| AiError::Network)?;
        if let Ok(data) = frame.into_data() {
            if bytes.len().saturating_add(data.len()) > 65536 {
                return Err(error);
            }
            bytes.extend_from_slice(&data);
        }
    }
    let quota = serde_json::from_slice::<serde_json::Value>(&bytes)
        .ok()
        .is_some_and(|value| {
            [value.pointer("/error/code"), value.pointer("/error/type")]
                .into_iter()
                .flatten()
                .filter_map(serde_json::Value::as_str)
                .any(|code| {
                    matches!(
                        code,
                        "insufficient_quota"
                            | "billing_hard_limit_reached"
                            | "credit_balance_too_low"
                    )
                })
        });
    Err(if quota { AiError::Quota } else { error })
}
