// SPDX-License-Identifier: Apache-2.0
//! Loopback only, default-off HTTP with strict Origin checks and bearer authentication.
use crate::{McpError, McpSettings, TokenStore, ToolBackend, server::ToolServer};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{Request, Response, StatusCode, body::Incoming, service::service_fn};
use hyper_util::rt::{TokioIo, TokioTimer};
use openrows_ai::{AiControl, sharing::SharingStore};
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use std::{
    convert::Infallible,
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};
use tokio::{net::TcpListener, sync::Semaphore};
use tokio_util::sync::CancellationToken;
use tower_service::Service;

type Body = http_body_util::combinators::BoxBody<Bytes, Infallible>;
/// App-owned host; the running listener never stores plaintext credentials.
#[derive(Clone)]
pub struct McpHost {
    /// Hash-only credential store and live cancellation.
    pub tokens: TokenStore,
    /// Shared M45-02 gate and audit.
    pub sharing: SharingStore,
    /// Live global AI permission (lock/off revoke this).
    pub control: AiControl,
    /// Native read/propose boundary.
    pub backend: Arc<dyn ToolBackend>,
}
/// RAII transport lifetime; dropping stops the listener, clients and in-flight work.
pub struct RunningServer {
    port: u16,
    cancel: CancellationToken,
    tokens: TokenStore,
}
impl RunningServer {
    /// Actual loopback port, safe to show in settings.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }
}
impl Drop for RunningServer {
    fn drop(&mut self) {
        self.cancel.cancel();
        self.tokens.cancel_all();
    }
}
impl McpHost {
    /// Start a bounded loopback bridge; direct HTTP remains separately opt-in.
    /// # Errors
    /// Disabled setting or loopback bind failure.
    pub async fn start(&self, settings: McpSettings) -> Result<RunningServer, McpError> {
        if !settings.enabled || !self.control.enabled() {
            return Err(McpError::Cancelled);
        }
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, settings.port))
            .await
            .map_err(|_| McpError::Transport)?;
        let port = listener
            .local_addr()
            .map_err(|_| McpError::Transport)?
            .port();
        let cancel = CancellationToken::new();
        let host = self.clone();
        tokio::spawn(accept(
            listener,
            host,
            settings.http_enabled,
            cancel.clone(),
        ));
        Ok(RunningServer {
            port,
            cancel,
            tokens: self.tokens.clone(),
        })
    }
    async fn request(&self, request: Request<Incoming>, direct: bool) -> Response<Body> {
        if !self.control.enabled() {
            return refusal(StatusCode::SERVICE_UNAVAILABLE);
        }
        let path = request.uri().path();
        if path != "/bridge" && !(direct && path == "/mcp") {
            return refusal(StatusCode::NOT_FOUND);
        }
        // No browser origin is needed by native MCP clients; reject all supplied origins.
        if request.headers().contains_key("origin") {
            return refusal(StatusCode::FORBIDDEN);
        }
        let credential = request
            .headers()
            .get("authorization")
            .and_then(|header| header.to_str().ok())
            .and_then(|text| text.strip_prefix("Bearer "));
        let Some(credential) = credential else {
            return refusal(StatusCode::UNAUTHORIZED);
        };
        let Ok(grant) = self.tokens.authenticate(credential) else {
            return refusal(StatusCode::UNAUTHORIZED);
        };
        let server = ToolServer {
            backend: self.backend.clone(),
            sharing: self.sharing.clone(),
            control: self.control.clone(),
            grant: Arc::new(grant),
        };
        let mut config = StreamableHttpServerConfig::default().enforce_origin_validation();
        config.legacy_session_mode = false;
        config.json_response = true;
        config.max_request_body_bytes = 32768;
        let mut service = StreamableHttpService::new(
            move || Ok(server.clone()),
            Arc::new(LocalSessionManager::default()),
            config,
        );
        match service.call(request).await {
            Ok(response) => response,
            Err(_) => refusal(StatusCode::BAD_REQUEST),
        }
    }
}
async fn accept(listener: TcpListener, host: McpHost, direct: bool, cancel: CancellationToken) {
    let slots = Arc::new(Semaphore::new(16));
    loop {
        tokio::select! {
            biased;
            () = cancel.cancelled() => break,
            accepted = listener.accept() => {
                let Ok((stream, peer)) = accepted else { break };
                if !peer.ip().is_loopback() { continue; }
                let Ok(slot) = slots.clone().try_acquire_owned() else { continue };
                let (host, cancel) = (host.clone(), cancel.clone());
                tokio::spawn(async move {
                    let _slot = slot;
                    let service = service_fn(move |request| { let host = host.clone(); async move { Ok::<_, Infallible>(host.request(request, direct).await) } });
                    let mut builder = hyper::server::conn::http1::Builder::new();
                    builder.timer(TokioTimer::new()).header_read_timeout(Duration::from_secs(2));
                    tokio::select! {
                        () = cancel.cancelled() => {},
                        _ = tokio::time::timeout(Duration::from_secs(30), builder.serve_connection(TokioIo::new(stream), service)) => {},
                    }
                });
            }
        }
    }
}
fn refusal(status: StatusCode) -> Response<Body> {
    let mut response = Response::new(Full::new(Bytes::new()).boxed());
    *response.status_mut() = status;
    response
}
/// Only explicit IPv4 loopback addresses are accepted by the CLI bridge.
/// # Errors
/// Non-loopback or zero-port target.
pub(crate) fn bridge_address(port: u16) -> Result<SocketAddr, McpError> {
    if port == 0 {
        return Err(McpError::Invalid);
    }
    Ok(SocketAddr::from((Ipv4Addr::LOCALHOST, port)))
}
