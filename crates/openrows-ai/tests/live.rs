// SPDX-License-Identifier: Apache-2.0
//! Explicit opt-in metadata-only smoke; never generates or reads database context.
use openrows_ai::{
    AiControl, Health, HttpProvider, NetworkOptions, Provider, ProviderConfig, ProviderKind,
    RequestOptions,
};
use openrows_secrets::SecretString;

#[tokio::test]
#[ignore = "requires OPENROWS_AI_LIVE=1 and the selected provider env key"]
async fn live_metadata_smoke() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("OPENROWS_AI_LIVE").as_deref() != Ok("1") {
        return Err("Set OPENROWS_AI_LIVE=1 explicitly".into());
    }
    let (kind, variable) = match std::env::var("OPENROWS_AI_PROVIDER").as_deref() {
        Ok("anthropic") => (ProviderKind::Anthropic, "ANTHROPIC_API_KEY"),
        Ok("openrouter") => (ProviderKind::OpenRouter, "OPENROUTER_API_KEY"),
        Ok("gemini") => (ProviderKind::Gemini, "GEMINI_API_KEY"),
        Ok("ollama") => (ProviderKind::Ollama, ""),
        _ => (ProviderKind::OpenAi, "OPENAI_API_KEY"),
    };
    let key = if variable.is_empty() {
        None
    } else {
        Some(SecretString::new(
            std::env::var(variable).map_err(|_| "selected provider key missing")?,
        ))
    };
    let config = ProviderConfig {
        id: "live".into(),
        name: "Live metadata".into(),
        kind,
        base_url: kind.default_url().into(),
        allow_insecure_remote: false,
        secret_ref: String::new(),
    };
    let control = AiControl::default();
    control.set_enabled(true);
    let result = HttpProvider::new(&config, key, control, NetworkOptions::default())?
        .verify(&RequestOptions::default())
        .await?;
    assert_eq!(result.health, Health::Verified);
    assert!(!result.models.is_empty());
    Ok(())
}
