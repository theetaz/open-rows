// SPDX-License-Identifier: Apache-2.0
//! Permission, endpoint and secret boundaries independent of the UI.
mod support;
use openrows_ai::{
    AiControl, AiError, AiSettings, Health, HttpProvider, NetworkOptions, Provider, ProviderKind,
    RequestOptions, TextRequest,
};
use openrows_secrets::{MemorySecretStore, SecretKey, SecretStore, SecretString};
use support::{Mock, TestResult, enabled, models, response};
use tokio::sync::mpsc;

#[tokio::test]
async fn ai_off_and_precancelled_make_zero_connections() -> TestResult {
    let mut mock = Mock::new(vec!["hang".into()]).await?;
    let control = AiControl::default();
    let provider = mock.provider(ProviderKind::OpenAi, control.clone())?;
    let options = RequestOptions::default();
    assert_eq!(provider.verify(&options).await, Err(AiError::Disabled));
    let (sink, _receiver) = mpsc::channel(1);
    let request = TextRequest {
        model: "model".into(),
        text: "hello".into(),
        max_output: 10,
    };
    assert_eq!(
        provider.stream(&request, sink, &options).await,
        Err(AiError::Disabled)
    );
    assert!(mock.requests.try_recv().is_err());
    control.set_enabled(true);
    options.cancel.cancel();
    assert_eq!(provider.verify(&options).await, Err(AiError::Cancelled));
    assert!(mock.requests.try_recv().is_err());
    Ok(())
}
#[tokio::test]
async fn disabling_revokes_an_active_request() -> TestResult {
    let mut mock = Mock::new(vec!["hang".into()]).await?;
    let control = enabled();
    let provider = mock.provider(ProviderKind::OpenAi, control.clone())?;
    let options = RequestOptions::default();
    let disable = async {
        mock.requests.recv().await;
        control.set_enabled(false);
        control.set_enabled(true);
    };
    let (result, ()) = tokio::join!(provider.verify(&options), disable);
    assert_eq!(result, Err(AiError::Disabled));
    Ok(())
}
#[tokio::test]
async fn settings_keychain_debug_errors_and_ipc_payloads_never_contain_key() -> TestResult {
    let mock = Mock::new(vec![models(ProviderKind::OpenAi)]).await?;
    let config = mock.config(ProviderKind::OpenAi);
    let settings = AiSettings {
        providers: vec![config.clone()],
        ..AiSettings::default()
    };
    let store = MemorySecretStore::new();
    let key = SecretString::from("test-only");
    store.set(&SecretKey::ai_provider(&config.id), &key)?;
    assert!(!serde_json::to_string(&settings)?.contains(key.expose()));
    assert!(!format!("{key:?}").contains(key.expose()));
    let verification = mock
        .provider(ProviderKind::OpenAi, enabled())?
        .verify(&RequestOptions::default())
        .await?;
    assert!(!serde_json::to_string(&verification)?.contains(key.expose()));
    assert!(serde_json::from_str::<AiSettings>(r#"{"apiKey":"test-only"}"#).is_err());
    let echo = Mock::new(vec![response(200, r#"{"data":[{"id":"test-only"}]}"#)]).await?;
    let error = echo
        .provider(ProviderKind::OpenAi, enabled())?
        .verify(&RequestOptions::default())
        .await
        .err()
        .ok_or("echo accepted")?;
    assert!(
        !format!("{error:?} {error} {}", serde_json::to_string(&error)?).contains(key.expose())
    );
    Ok(())
}
#[tokio::test]
async fn endpoint_policy_refuses_credentials_queries_remote_plaintext_and_local_remote()
-> TestResult {
    let mock = Mock::new(Vec::new()).await?;
    let mut config = mock.config(ProviderKind::Compatible);
    for url in [
        "http://example.com/",
        "https://user:pw@example.com/",
        "https://example.com/?key=x",
        "https://example.com/#x",
        "file:///tmp/x",
    ] {
        config.base_url = url.into();
        assert_eq!(config.validate(), Err(AiError::InvalidConfig));
    }
    config.base_url = "https://example.com/v1".into();
    assert!(config.validate().is_ok());
    config.kind = ProviderKind::Ollama;
    assert_eq!(config.validate(), Err(AiError::InvalidConfig));
    config.allow_insecure_remote = true;
    assert!(config.validate().is_ok());
    config.base_url = "http://example.com/".into();
    assert!(config.validate().is_ok());
    Ok(())
}
#[tokio::test]
async fn anthropic_fallback_and_unreported_capabilities_stay_unknown() -> TestResult {
    let mock = Mock::new(vec![response(404, "")]).await?;
    let result = mock
        .provider(ProviderKind::Anthropic, enabled())?
        .verify(&RequestOptions::default())
        .await?;
    assert_eq!(result.health, Health::Unknown);
    assert_eq!(result.models[0].id, "claude-opus-5-5");
    assert_eq!(result.models[0].capabilities.context_window, None);
    assert_eq!(result.usage, None);
    Ok(())
}
#[tokio::test]
async fn keyless_local_models_and_redirects() -> TestResult {
    let mock = Mock::new(vec![
        models(ProviderKind::Ollama),
        response(302, "test-only"),
    ])
    .await?;
    let provider = HttpProvider::new(
        &mock.config(ProviderKind::Ollama),
        None,
        enabled(),
        NetworkOptions::default(),
    )?;
    assert_eq!(
        provider.verify(&RequestOptions::default()).await?.health,
        Health::Verified
    );
    assert_eq!(
        provider.verify(&RequestOptions::default()).await,
        Err(AiError::Network)
    );
    Ok(())
}

#[tokio::test]
async fn quota_codes_and_model_pagination_are_scrubbed() -> TestResult {
    let mock = Mock::new(vec![response(
        429,
        r#"{"error":{"code":"insufficient_quota","message":"test-only"}}"#,
    )])
    .await?;
    assert_eq!(
        mock.provider(ProviderKind::OpenAi, enabled())?
            .verify(&RequestOptions::default())
            .await,
        Err(AiError::Quota)
    );
    let mut mock = Mock::new(vec![
        response(
            200,
            r#"{"models":[{"name":"models/first"}],"nextPageToken":"next /?"}"#,
        ),
        response(200, r#"{"models":[{"name":"models/second"}]}"#),
    ])
    .await?;
    let result = mock
        .provider(ProviderKind::Gemini, enabled())?
        .verify(&RequestOptions::default())
        .await?;
    assert_eq!(result.models.len(), 2);
    mock.requests.recv().await;
    let next = mock.requests.recv().await.ok_or("second page missing")?;
    assert!(next.contains("pageToken=next+%2F%3F"));
    Ok(())
}
#[tokio::test]
async fn stream_backpressure_deadline_and_cancellation() -> TestResult {
    let mock = Mock::new(vec![support::stream(ProviderKind::OpenAi)]).await?;
    let provider = mock.provider(ProviderKind::OpenAi, enabled())?;
    let (sink, _receiver) = mpsc::channel(1);
    sink.send("occupied".into()).await?;
    let options = RequestOptions {
        timeout: std::time::Duration::from_millis(30),
        ..RequestOptions::default()
    };
    let request = TextRequest {
        model: "model".into(),
        text: "hello".into(),
        max_output: 10,
    };
    assert_eq!(
        provider.stream(&request, sink, &options).await,
        Err(AiError::Timeout)
    );
    let mut mock = Mock::new(vec!["hang".into()]).await?;
    let provider = mock.provider(ProviderKind::OpenAi, enabled())?;
    let (sink, _receiver) = mpsc::channel(1);
    let options = RequestOptions::default();
    let cancel = async {
        mock.requests.recv().await;
        options.cancel.cancel();
    };
    let (result, ()) = tokio::join!(provider.stream(&request, sink, &options), cancel);
    assert_eq!(result, Err(AiError::Cancelled));
    Ok(())
}

struct FailIfDialed(std::sync::atomic::AtomicU32);
impl openrows_net::Dialer for FailIfDialed {
    fn dial(&self) -> openrows_net::BoxFuture<'_, std::io::Result<openrows_net::BoxStream>> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Box::pin(async { Err(std::io::Error::other("must never be contacted while off")) })
    }
    fn describe(&self) -> String {
        "fail-if-contacted".into()
    }
}
#[tokio::test]
async fn ai_off_never_even_polls_the_outbound_dialer() -> TestResult {
    let mock = Mock::new(Vec::new()).await?;
    let dialer = std::sync::Arc::new(FailIfDialed(std::sync::atomic::AtomicU32::new(0)));
    let control = AiControl::default();
    let network = NetworkOptions {
        route: Some(dialer.clone()),
        ..NetworkOptions::default()
    };
    let provider = HttpProvider::new(
        &mock.config(ProviderKind::OpenAi),
        Some(SecretString::from("test-only")),
        control.clone(),
        network,
    )?;
    assert_eq!(
        provider.verify(&RequestOptions::default()).await,
        Err(AiError::Disabled)
    );
    assert_eq!(dialer.0.load(std::sync::atomic::Ordering::SeqCst), 0);
    control.set_enabled(true);
    assert_eq!(
        provider.verify(&RequestOptions::default()).await,
        Err(AiError::Network)
    );
    assert_eq!(dialer.0.load(std::sync::atomic::Ordering::SeqCst), 1);
    Ok(())
}
