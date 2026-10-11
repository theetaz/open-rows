// SPDX-License-Identifier: Apache-2.0
//! One protocol adapter per configured provider, behind a Tauri-free trait.
use crate::{
    AiControl, AiError, NetworkOptions, ProviderConfig, ProviderKind, RequestOptions, Verification,
    http::Http,
};
use openrows_net::BoxFuture;
use openrows_secrets::SecretString;
use tokio::sync::mpsc;

/// Generation input accepts only context constructed by the data-sharing gate.
pub struct TextRequest {
    /// Explicit discovered or user-entered model identifier.
    pub model: String,
    /// Exact authorised context; no raw database text or deserialisation escape.
    pub context: crate::sharing::GatedPayload,
    /// Output token cap, required by every adapter.
    pub max_output: u32,
}
/// Provider metadata and bounded incremental text transport.
pub trait Provider: Send + Sync {
    /// Probe only auth/models/key metadata; never generate text.
    fn verify<'a>(
        &'a self,
        options: &'a RequestOptions,
    ) -> BoxFuture<'a, Result<Verification, AiError>>;
    /// Stream text to a bounded consumer channel under the same permission gate.
    fn stream<'a>(
        &'a self,
        request: &'a TextRequest,
        sink: mpsc::Sender<String>,
        options: &'a RequestOptions,
    ) -> BoxFuture<'a, Result<(), AiError>>;
}
/// Native HTTP adapters reuse the existing network routes and TLS stack.
pub struct HttpProvider {
    pub(crate) http: Http,
    pub(crate) kind: ProviderKind,
    control: AiControl,
    config: ProviderConfig,
    pub(crate) tools: bool,
}
impl HttpProvider {
    /// Build an adapter without connecting; keys are neither serializable nor printable.
    /// # Errors
    /// Rejects invalid endpoint/configuration and TLS settings.
    pub fn new(
        config: &ProviderConfig,
        key: Option<SecretString>,
        control: AiControl,
        network: NetworkOptions,
    ) -> Result<Self, AiError> {
        let http = Http::new(config.validate()?, config.kind, key, network)?;
        Ok(Self {
            http,
            kind: config.kind,
            control,
            config: config.clone(),
            tools: false,
        })
    }
}
impl HttpProvider {
    /// Use explicitly verified model capability; unknown models stay single-shot.
    #[must_use]
    pub fn with_tool_calling(mut self, supported: bool) -> Self {
        self.tools = supported;
        self
    }
}
impl crate::agent::AgentProvider for HttpProvider {
    fn tool_calling(&self) -> bool {
        self.tools
    }
    fn turn<'a>(
        &'a self,
        request: crate::agent::TurnRequest<'a>,
        sink: mpsc::Sender<String>,
        options: &'a RequestOptions,
    ) -> BoxFuture<'a, Result<crate::agent::Turn, AiError>> {
        Box::pin(async move {
            self.control
                .run(
                    options,
                    request
                        .context
                        .outbound(&self.config, crate::streaming::turn(self, &request, sink)),
                )
                .await
        })
    }
}
impl Provider for HttpProvider {
    fn verify<'a>(
        &'a self,
        options: &'a RequestOptions,
    ) -> BoxFuture<'a, Result<Verification, AiError>> {
        Box::pin(self.control.run(options, crate::models::verify(self)))
    }
    fn stream<'a>(
        &'a self,
        request: &'a TextRequest,
        sink: mpsc::Sender<String>,
        options: &'a RequestOptions,
    ) -> BoxFuture<'a, Result<(), AiError>> {
        Box::pin(
            self.control.run(
                options,
                request
                    .context
                    .outbound(&self.config, crate::streaming::stream(self, request, sink)),
            ),
        )
    }
}
