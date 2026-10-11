// SPDX-License-Identifier: Apache-2.0
//! Explicit live generation smoke using synthetic context and no database executor.
mod agent_support;
use agent_support::{Result, Tools, run};
use openrows_ai::{HttpProvider, NetworkOptions, ProviderKind, agent::AgentEvent};
use openrows_secrets::SecretString;
#[tokio::test]
#[ignore = "requires OPENROWS_AI_LIVE=1, OPENROWS_AI_MODEL and selected provider key"]
async fn live_assistant_single_shot() -> Result {
    if std::env::var("OPENROWS_AI_LIVE").as_deref() != Ok("1") {
        return Err("Set OPENROWS_AI_LIVE=1 explicitly".into());
    }
    let (kind, variable) = match std::env::var("OPENROWS_AI_PROVIDER").as_deref() {
        Ok("anthropic") => (ProviderKind::Anthropic, "ANTHROPIC_API_KEY"),
        Ok("openrouter") => (ProviderKind::OpenRouter, "OPENROUTER_API_KEY"),
        Ok("gemini") => (ProviderKind::Gemini, "GEMINI_API_KEY"),
        Ok("ollama") => (ProviderKind::Ollama, ""),
        Ok("openai") | Err(_) => (ProviderKind::OpenAi, "OPENAI_API_KEY"),
        _ => return Err("Unsupported live provider".into()),
    };
    let key = if variable.is_empty() {
        None
    } else {
        Some(SecretString::new(
            std::env::var(variable).map_err(|_| "selected provider key missing")?,
        ))
    };
    let mut tools = Tools::new()?;
    tools.config.kind = kind;
    tools.config.base_url = kind.default_url().into();
    let provider = HttpProvider::new(
        &tools.config,
        key,
        tools.control.clone(),
        NetworkOptions::default(),
    )?;
    let mut request = tools.request()?;
    request.model = std::env::var("OPENROWS_AI_MODEL").map_err(|_| "OPENROWS_AI_MODEL missing")?;
    let (result, events) = run(&provider, &tools, &request).await;
    result?;
    assert!(
        events
            .iter()
            .any(|event| matches!(event, AgentEvent::Text { delta } if !delta.is_empty()))
    );
    assert!(!tools.store.audit()?.is_empty());
    assert_eq!(tools.count.load(std::sync::atomic::Ordering::SeqCst), 0);
    Ok(())
}
