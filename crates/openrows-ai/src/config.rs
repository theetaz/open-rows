// SPDX-License-Identifier: Apache-2.0
//! Serializable settings contain keychain references only, never secret values.
use crate::AiError;
use serde::{Deserialize, Serialize};
use url::Url;

/// Supported provider wire protocols.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ProviderKind {
    /// Anthropic Messages.
    Anthropic,
    /// `OpenAI` Responses (with server-side storage disabled).
    OpenAi,
    /// `OpenRouter` Chat Completions plus key metadata.
    OpenRouter,
    /// Google Gemini REST.
    Gemini,
    /// A custom Chat Completions endpoint.
    Compatible,
    /// Ollama native chat/tags.
    Ollama,
    /// Loopback Chat Completions (LM Studio, llama.cpp, etc.).
    LocalCompatible,
}
impl ProviderKind {
    /// Whether the endpoint must stay loopback unless explicitly opted out.
    #[must_use]
    pub fn is_local(self) -> bool {
        matches!(self, Self::Ollama | Self::LocalCompatible)
    }
    /// Suggested endpoint; models are always discovered separately.
    #[must_use]
    pub fn default_url(self) -> &'static str {
        match self {
            Self::Anthropic => "https://api.anthropic.com/v1/",
            Self::OpenAi => "https://api.openai.com/v1/",
            Self::OpenRouter => "https://openrouter.ai/api/v1/",
            Self::Gemini => "https://generativelanguage.googleapis.com/v1beta/",
            Self::Compatible => "",
            Self::Ollama => "http://127.0.0.1:11434/",
            Self::LocalCompatible => "http://127.0.0.1:1234/v1/",
        }
    }
}

/// One provider's non-secret configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderConfig {
    /// Stable bounded identifier used for keychain addressing.
    pub id: String,
    /// User label.
    pub name: String,
    /// Wire protocol.
    pub kind: ProviderKind,
    /// Endpoint root, without credentials, query or fragment.
    pub base_url: String,
    /// Explicit warning acknowledgement for remote plaintext/local overrides.
    pub allow_insecure_remote: bool,
    /// Canonical keychain reference; empty for keyless local servers.
    pub secret_ref: String,
}
impl ProviderConfig {
    /// Validate configuration before storing or connecting.
    /// # Errors
    /// Returns a fixed error for unsafe URLs or invalid references.
    pub fn validate(&self) -> Result<Url, AiError> {
        validate_id(&self.id)?;
        if self.name.trim().is_empty() || self.name.len() > 80 || self.base_url.len() > 2048 {
            return Err(AiError::InvalidConfig);
        }
        if !self.secret_ref.is_empty() && self.secret_ref != format!("ai/{}/api-key", self.id) {
            return Err(AiError::InvalidConfig);
        }
        let mut url = Url::parse(&self.base_url).map_err(|_| AiError::InvalidConfig)?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(AiError::InvalidConfig);
        }
        let local = match url.host() {
            Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
            Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
            Some(url::Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
            None => false,
        };
        if !local
            && !self.allow_insecure_remote
            && (self.kind.is_local() || url.scheme() != "https")
        {
            return Err(AiError::InvalidConfig);
        }
        if !url.path().ends_with('/') {
            url.set_path(&format!("{}/", url.path()));
        }
        Ok(url)
    }
}
/// Validate an identifier before any keychain operation.
/// # Errors
/// Returns `InvalidConfig` for invalid or unbounded identifiers.
pub fn validate_id(id: &str) -> Result<(), AiError> {
    if id.is_empty() || id.len() > 64 || !id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
    {
        return Err(AiError::InvalidConfig);
    }
    Ok(())
}

/// Device AI preferences; the owner must explicitly enable network operations.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct AiSettings {
    /// Global permission (default off).
    pub enabled: bool,
    /// Multiple independent saved providers.
    pub providers: Vec<ProviderConfig>,
    /// Explicit default provider, empty when unset.
    pub default_provider: String,
    /// Explicit default model, empty when unset.
    pub default_model: String,
}
impl AiSettings {
    /// Validate every provider and default without issuing a network request.
    /// # Errors
    /// Returns `InvalidConfig` for duplicates, invalid defaults or oversized input.
    pub fn validate(&self) -> Result<(), AiError> {
        if self.providers.len() > 32 || self.default_model.len() > 256 {
            return Err(AiError::InvalidConfig);
        }
        let mut ids = std::collections::HashSet::new();
        for provider in &self.providers {
            provider.validate()?;
            if !ids.insert(&provider.id) {
                return Err(AiError::InvalidConfig);
            }
        }
        if !self.default_provider.is_empty() && !ids.contains(&self.default_provider) {
            return Err(AiError::InvalidConfig);
        }
        Ok(())
    }
}
