// SPDX-License-Identifier: Apache-2.0
//! The only constructor for provider database context; previews cannot be promoted.
use super::{
    AuditEntry, ContextBundle, ContextPreview, Limits, Refusal, SharingLevel, SharingStore,
};
use crate::{AiControl, AiError, ProviderConfig};
use openrows_sql::Dialect;
use sha2::{Digest, Sha256};
use std::{
    future::Future,
    time::{SystemTime, UNIX_EPOCH},
};

/// All trusted identity/permission inputs for one context request.
pub struct GateRequest<'a> {
    /// Saved connection ID (native caller resolves production and dialect).
    pub connection: &'a str,
    /// Current production tag, resolved from the stored profile.
    pub production: bool,
    /// Saved provider, not caller-supplied endpoint/protocol claims.
    pub provider: &'a ProviderConfig,
    /// Live global permission and revocation generation.
    pub control: &'a AiControl,
    /// Engine's SQL lexer.
    pub dialect: Dialect,
    /// Selected categories, never sent directly to a provider.
    pub bundle: ContextBundle,
}

/// Non-serializable/non-deserializable provider context, created only by the gate.
/// ```compile_fail
/// use openrows_ai::sharing::GatedPayload;
/// let payload: GatedPayload = serde_json::from_str("{}").unwrap();
/// ```
/// ```compile_fail
/// use openrows_ai::{TextRequest, sharing::ContextBundle};
/// let request = TextRequest { model: "model".into(), context: ContextBundle::default(), max_output: 10 };
/// ```
pub struct GatedPayload {
    preview: ContextPreview,
    connection: String,
    provider: ProviderConfig,
    level: SharingLevel,
    store: SharingStore,
    revision: u64,
    control: AiControl,
    generation: u64,
}
impl GatedPayload {
    /// Display exactly the bytes that the provider transport will use.
    #[must_use]
    pub fn preview(&self) -> ContextPreview {
        self.preview.clone()
    }
    pub(crate) fn text(&self) -> &str {
        &self.preview.payload
    }
    /// Recheck sharing and permission revisions throughout an outbound operation.
    /// # Errors
    /// Revocation or the operation failure.
    pub async fn outbound<T>(
        &self,
        provider: &ProviderConfig,
        work: impl Future<Output = Result<T, AiError>>,
    ) -> Result<T, AiError> {
        let mut revision = self.store.revision.subscribe();
        if provider != &self.provider
            || *revision.borrow() != self.revision
            || !self.control.current(self.generation)
        {
            return Err(AiError::Disabled);
        }
        tokio::select! {
            biased;
            _ = revision.wait_for(|value| *value != self.revision) => Err(AiError::Disabled),
            () = self.control.revoked(self.generation) => Err(AiError::Disabled),
            result = work => result,
        }
    }
    /// Append mandatory metadata before releasing a tool result.
    /// # Errors
    /// Audit storage failures refuse the outbound response.
    pub async fn record(&self) -> Result<(), AiError> {
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| AiError::InvalidConfig)?
            .as_millis()
            .to_string();
        let entry = AuditEntry {
            time,
            provider: self.provider.id.clone(),
            connection: self.connection.clone(),
            level: self.level,
            bytes: self.preview.bytes,
            hash: format!("{:x}", Sha256::digest(self.text().as_bytes())),
            categories: self.preview.categories.clone(),
        };
        let store = self.store.clone();
        tokio::task::spawn_blocking(move || store.append(&entry))
            .await
            .map_err(|_| AiError::Audit)?
            .map_err(|_| AiError::Audit)
    }
}
impl SharingStore {
    /// Authorise and redact requested database context using the default hard caps.
    /// # Errors
    /// Fixed typed refusal; no partial context escapes a denied request.
    pub fn gate(&self, request: GateRequest<'_>) -> Result<GatedPayload, Refusal> {
        self.gate_with_limits(request, Limits::default())
    }
    /// Authorise with tighter caller bounds; hard defaults can never be increased.
    /// # Errors
    /// Fixed policy refusal or invalid limits/context.
    pub fn gate_with_limits(
        &self,
        request: GateRequest<'_>,
        limits: Limits,
    ) -> Result<GatedPayload, Refusal> {
        validate_limits(limits)?;
        let revision = *self.revision.borrow();
        let generation = request.control.generation();
        let policy = self.policy(request.connection)?;
        authorize(&request, &policy)?;
        let bundle = &request.bundle;
        if bundle.schema.len() > 128
            || bundle.plan.len() > 200
            || bundle.stats.len() > 200
            || bundle.samples.len() > 20
            || bundle
                .samples
                .iter()
                .any(|sample| sample.columns.len() > 200)
        {
            return Err(Refusal::Invalid);
        }
        request.bundle.validate_size()?;
        let mut redactor = super::redact::Redactor::new(&policy.rules, limits, request.dialect)?;
        let mut bundle = redactor.bundle(request.bundle)?;
        let (payload, shortened) = fit(&mut bundle, limits.bytes.min(limits.tokens))?;
        let truncated = redactor.truncated || shortened;
        let bytes = u32::try_from(payload.len()).map_err(|_| Refusal::Invalid)?;
        Ok(GatedPayload {
            preview: ContextPreview {
                payload,
                bytes,
                tokens: bytes,
                categories: bundle.categories(),
                truncated,
            },
            connection: request.connection.into(),
            provider: request.provider.clone(),
            level: policy.level,
            store: self.clone(),
            revision,
            control: request.control.clone(),
            generation,
        })
    }
}
fn authorize(request: &GateRequest<'_>, policy: &super::SharingPolicy) -> Result<(), Refusal> {
    if !request.control.enabled() {
        return Err(Refusal::AiOff);
    }
    if request.production && !policy.production_confirmed {
        return Err(Refusal::Production);
    }
    let bundle = &request.bundle;
    if policy.level == SharingLevel::None
        || (!bundle.samples.is_empty() && policy.level < SharingLevel::SampleRows)
        || ((!bundle.stats.is_empty() || !bundle.plan.is_empty())
            && policy.level < SharingLevel::SchemaAndStats)
    {
        return Err(Refusal::LevelTooLow);
    }
    if !policy.providers.contains(&request.provider.id) {
        return Err(Refusal::ProviderNotAllowed);
    }
    let url = request.provider.validate().map_err(|_| Refusal::Invalid)?;
    let loopback = match url.host() {
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        Some(url::Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
        None => false,
    };
    if policy.local_only && (!request.provider.kind.is_local() || !loopback) {
        return Err(Refusal::LocalOnly);
    }
    if (!bundle.stats.is_empty() || !bundle.plan.is_empty()) && !policy.telemetry {
        return Err(Refusal::Telemetry);
    }
    Ok(())
}
fn validate_limits(limits: Limits) -> Result<(), Refusal> {
    let hard = Limits::default();
    if limits.rows > hard.rows
        || limits.cells > hard.cells
        || limits.field_bytes > hard.field_bytes
        || limits.bytes > hard.bytes
        || limits.tokens > hard.tokens
        || limits.field_bytes < "‹redacted›".len()
        || limits.bytes < 64
        || limits.tokens < 64
    {
        return Err(Refusal::Invalid);
    }
    Ok(())
}

fn fit(bundle: &mut ContextBundle, cap: usize) -> Result<(String, bool), Refusal> {
    let payload = serde_json::to_string(bundle).map_err(|_| Refusal::Invalid)?;
    if payload.len() <= cap {
        return Ok((payload, false));
    }
    let mut bytes = payload.len();
    while bytes > cap {
        let removed = bundle.shrink()?.ok_or(Refusal::Invalid)?;
        if removed == 0 {
            return Err(Refusal::Invalid);
        }
        bytes -= removed;
    }
    let payload = serde_json::to_string(bundle).map_err(|_| Refusal::Invalid)?;
    if payload.len() > cap {
        return Err(Refusal::Invalid);
    }
    Ok((payload, true))
}
