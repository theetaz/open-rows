// SPDX-License-Identifier: Apache-2.0
//! One protocol adapter per configured provider, behind a Tauri-free trait.
use crate::{
    AiControl, AiError, NetworkOptions, ProviderConfig, ProviderKind, RequestOptions, Verification,
    http::Http,
};
use openrows_net::BoxFuture;
use openrows_secrets::SecretString;
use tokio::sync::mpsc;

/// Text-only generation input; this task exposes no database/context IPC command.
pub struct TextRequest {
    /// Explicit discovered or user-entered model identifier.
    pub model: String,
    /// User text; later context sharing belongs to M45-02.
    pub text: String,
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
            self.control
                .run(options, crate::streaming::stream(self, request, sink)),
        )
    }
}
