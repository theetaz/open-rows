// SPDX-License-Identifier: Apache-2.0
//! Stdio sidecar forwards only protocol calls to the authenticated running app.
use crate::{McpError, http::bridge_address};
use bytes::Bytes;
use http_body_util::{BodyExt, Full, Limited};
use hyper_util::rt::TokioIo;
use rmcp::{
    RoleServer, ServerHandler, ServiceExt,
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, Implementation, ListToolsResult,
        PaginatedRequestParams, ServerCapabilities, ServerConfig,
    },
    service::RequestContext,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::json;
use std::time::Duration;
use tokio::net::TcpStream;

struct Bridge {
    port: u16,
    credential: String,
}
impl ServerHandler for Bridge {
    fn get_info(&self) -> ServerConfig {
        let mut config = ServerConfig::default();
        config.capabilities = ServerCapabilities::builder().enable_tools().build();
        config
    }
    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, rmcp::ErrorData> {
        self.forward("tools/list", json!({}), context).await
    }
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, rmcp::ErrorData> {
        self.forward::<CallToolResult>("tools/call", request, context)
            .await
            .map(Into::into)
    }
}
impl Bridge {
    async fn forward<T: DeserializeOwned>(
        &self,
        method: &str,
        params: impl Serialize + Send,
        context: RequestContext<RoleServer>,
    ) -> Result<T, rmcp::ErrorData> {
        tokio::select! {
            () = context.ct.cancelled() => Err(protocol_error(McpError::Cancelled)),
            result = tokio::time::timeout(Duration::from_secs(6), self.post(method, params, context.client_info())) => {
                result.map_err(|_| protocol_error(McpError::Limit))?.map_err(protocol_error)
            }
        }
    }
}
/// Serve MCP over stdio, forwarding to an explicitly selected app loopback port.
/// Credentials must come from the spawning client's environment, never argv or settings.
/// # Errors
/// Fixed protocol/transport errors without credentials or remote response text.
pub async fn stdio_bridge(port: u16, credential: String) -> Result<(), McpError> {
    bridge_address(port)?;
    if credential.len() != 64 {
        return Err(McpError::Auth);
    }
    let bridge = Bridge { port, credential };
    let _: ListToolsResult = tokio::time::timeout(
        Duration::from_secs(6),
        bridge.post("tools/list", json!({}), None),
    )
    .await
    .map_err(|_| McpError::Limit)??;
    let service = bridge
        .serve(rmcp::transport::stdio())
        .await
        .map_err(|_| McpError::Transport)?;
    service.waiting().await.map_err(|_| McpError::Transport)?;
    Ok(())
}
impl Bridge {
    /// Forward one request; the downstream client's self-reported identity
    /// (name and version only) labels activity in the app.
    async fn post<T: DeserializeOwned>(
        &self,
        method: &str,
        params: impl Serialize,
        client: Option<Implementation>,
    ) -> Result<T, McpError> {
        let mut params = serde_json::to_value(params).map_err(|_| McpError::Invalid)?;
        let client = client.map_or_else(
            || json!({"name":"openrows-mcp","version":"0.1.0"}),
            |info| json!({"name": short(&info.name), "version": short(&info.version)}),
        );
        params["_meta"] = json!({"io.modelcontextprotocol/protocolVersion":"2026-07-28", "io.modelcontextprotocol/clientInfo":client, "io.modelcontextprotocol/clientCapabilities":{}});
        send(self.port, &self.credential, method, params).await
    }
}
async fn send<T: DeserializeOwned>(
    port: u16,
    credential: &str,
    method: &str,
    params: serde_json::Value,
) -> Result<T, McpError> {
    let message = json!({"jsonrpc":"2.0","id":1,"method":method,"params":params});
    let stream = TcpStream::connect(bridge_address(port)?)
        .await
        .map_err(|_| McpError::Transport)?;
    let (mut sender, connection) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
        .await
        .map_err(|_| McpError::Transport)?;
    let task = tokio::spawn(async move {
        let _ = connection.await;
    });
    let guard = Abort(task);
    let request = request(port, credential, &message)?;
    let response = sender
        .send_request(request)
        .await
        .map_err(|_| McpError::Transport)?;
    if !response.status().is_success() {
        return Err(McpError::Auth);
    }
    let bytes = Limited::new(response.into_body(), 131_072)
        .collect()
        .await
        .map_err(|_| McpError::Limit)?
        .to_bytes();
    let response: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| McpError::Transport)?;
    drop(guard);
    serde_json::from_value(response.get("result").cloned().ok_or(McpError::Transport)?)
        .map_err(|_| McpError::Transport)
}
fn request(
    port: u16,
    credential: &str,
    message: &serde_json::Value,
) -> Result<hyper::Request<Full<Bytes>>, McpError> {
    let mut authorization = hyper::header::HeaderValue::from_str(&format!("Bearer {credential}"))
        .map_err(|_| McpError::Invalid)?;
    authorization.set_sensitive(true);
    let method = message["method"].as_str().ok_or(McpError::Invalid)?;
    let mut request = hyper::Request::builder()
        .method("POST")
        .uri(format!("http://127.0.0.1:{port}/bridge"))
        .header("authorization", authorization)
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .header("mcp-protocol-version", "2026-07-28")
        .header("mcp-method", method);
    if let Some(name) = message["params"]["name"].as_str() {
        request = request.header("mcp-name", name);
    }
    request
        .body(Full::new(Bytes::from(
            serde_json::to_vec(message).map_err(|_| McpError::Invalid)?,
        )))
        .map_err(|_| McpError::Invalid)
}
struct Abort(tokio::task::JoinHandle<()>);
impl Drop for Abort {
    fn drop(&mut self) {
        self.0.abort();
    }
}
fn short(text: &str) -> String {
    text.chars().take(64).collect()
}
fn protocol_error(error: McpError) -> rmcp::ErrorData {
    rmcp::ErrorData::internal_error(error.to_string(), None)
}
