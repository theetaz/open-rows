// SPDX-License-Identifier: Apache-2.0
use super::*;
#[test]
fn secrets_are_rejected_by_settings_and_ipc_errors_are_fixed() {
    assert!(validate_settings(&serde_json::json!({"ai":{"apiKey":"test-only"}})).is_err());
    assert!(
        validate_settings(&serde_json::json!({"ai":{"providers":[{"key":"test-only"}]}})).is_err()
    );
    let error = IpcError::from(AiError::Auth);
    assert_eq!(error.kind, IpcErrorKind::Auth);
    assert!(!serde_json::to_string(&error).unwrap().contains("test-only"));
}
#[test]
fn global_permission_defaults_off_and_invalid_settings_revoke_it() {
    let state = AiState::default();
    assert!(!state.control.enabled());
    state.configure(&serde_json::json!({"ai":{"enabled":true}}));
    assert!(state.control.enabled());
    state.configure(&serde_json::json!({"ai":{"enabled":true,"key":"test-only"}}));
    assert!(!state.control.enabled());
}
fn provider(kind: openrows_ai::ProviderKind, secret_ref: &str) -> ProviderConfig {
    ProviderConfig {
        id: "fixture".into(),
        name: "Fixture".into(),
        kind,
        base_url: "https://api.anthropic.com/v1/".into(),
        allow_insecure_remote: false,
        secret_ref: secret_ref.into(),
    }
}
#[tokio::test]
async fn prompted_keys_go_to_the_keychain_and_cancel_keeps_the_old_one() -> Result<(), IpcError> {
    use crate::security::secret_prompt::tests::FakePrompt;
    let store = Arc::new(openrows_secrets::MemorySecretStore::new());
    let config = provider(openrows_ai::ProviderKind::Anthropic, "ai/fixture/api-key");
    let saved = prompt_key(&FakePrompt::new(Some("test-only")), store.clone(), &config).await?;
    assert_eq!(saved, SecretEntry::Saved);
    let key = SecretKey::ai_provider("fixture");
    let read = |store: &openrows_secrets::MemorySecretStore| {
        store
            .get(&key)
            .ok()
            .flatten()
            .map(|value| value.expose().to_owned())
    };
    assert_eq!(read(&store).as_deref(), Some("test-only"));
    let cancelled = prompt_key(&FakePrompt::new(None), store.clone(), &config).await?;
    assert_eq!(cancelled, SecretEntry::Cancelled);
    assert_eq!(read(&store).as_deref(), Some("test-only"));
    Ok(())
}
#[tokio::test]
async fn keyless_and_cli_providers_never_open_the_prompt() {
    use crate::security::secret_prompt::tests::FakePrompt;
    let store = Arc::new(openrows_secrets::MemorySecretStore::new());
    for config in [
        provider(openrows_ai::ProviderKind::Ollama, ""),
        provider(openrows_ai::ProviderKind::Codex, "ai/fixture/api-key"),
    ] {
        let prompt = FakePrompt::new(Some("test-only"));
        assert!(prompt_key(&prompt, store.clone(), &config).await.is_err());
        assert!(prompt.asked.lock().is_ok_and(|asked| asked.is_empty()));
    }
}
