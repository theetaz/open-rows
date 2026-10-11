// SPDX-License-Identifier: Apache-2.0
//! Real loopback outbound requests prove mandatory, content-free audit and revocation.
mod support;
use openrows_ai::{AiError, Provider, ProviderKind, RequestOptions, TextRequest, sharing::*};
use openrows_sql::Dialect;
use support::{Mock, TestResult, enabled};
use tokio::sync::mpsc;
fn gated(
    store: &SharingStore,
    mock: &Mock,
    control: &openrows_ai::AiControl,
) -> Result<GatedPayload, Refusal> {
    store.gate(GateRequest {
        connection: "connection",
        production: false,
        provider: &mock.config(ProviderKind::OpenAi),
        control,
        dialect: Dialect::Postgres,
        bundle: ContextBundle {
            samples: vec![Sample {
                table: "t".into(),
                columns: vec!["email".into(), "value".into()],
                rows: vec![vec![
                    Some("private-email".into()),
                    Some("private-row".into()),
                ]],
            }],
            ..ContextBundle::default()
        },
    })
}
fn policy() -> SharingPolicy {
    SharingPolicy {
        level: SharingLevel::SampleRows,
        providers: vec!["fixture".into()],
        ..SharingPolicy::default()
    }
}
#[tokio::test]
async fn every_outbound_attempt_records_hash_size_categories_and_no_content_or_keys() -> TestResult
{
    let store = SharingStore::memory(2)?;
    store.save("connection", policy(), false)?;
    let control = enabled();
    let mut mock = Mock::new(vec![support::stream(ProviderKind::OpenAi); 3]).await?;
    let context = gated(&store, &mock, &control)?;
    let preview = context.preview();
    assert!(store.audit()?.is_empty());
    let request = TextRequest {
        model: "model".into(),
        context,
        max_output: 10,
    };
    let provider = mock.provider(ProviderKind::OpenAi, control)?;
    for _ in 0..3 {
        let (sink, _receiver) = mpsc::channel(2);
        provider
            .stream(&request, sink, &RequestOptions::default())
            .await?;
        let wire = mock.requests.recv().await.ok_or("missing request")?;
        let body = wire.split_once("\r\n\r\n").ok_or("missing body")?.1;
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(body)?["input"],
            preview.payload
        );
    }
    let entries = store.audit()?;
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].bytes, preview.bytes);
    assert_eq!(entries[0].hash.len(), 64);
    assert_eq!(entries[0].categories, vec![Category::Samples]);
    let metadata = serde_json::to_string(&entries)?;
    for forbidden in [
        "private-row",
        "private-email",
        "test-only",
        "api-key",
        "payload",
        "email",
        "model",
        "https://",
    ] {
        assert!(!metadata.contains(forbidden));
    }
    store.purge()?;
    assert!(store.audit()?.is_empty());
    assert_eq!(store.policy("connection")?.level, SharingLevel::SampleRows);
    Ok(())
}
#[tokio::test]
async fn changed_policy_and_changed_endpoint_refuse_old_payload_before_dial() -> TestResult {
    let store = SharingStore::memory(1000)?;
    store.save("connection", policy(), false)?;
    let control = enabled();
    let mut mock = Mock::new(vec!["hang".into()]).await?;
    let request = TextRequest {
        model: "model".into(),
        context: gated(&store, &mock, &control)?,
        max_output: 10,
    };
    store.save("connection", SharingPolicy::default(), false)?;
    let (sink, _receiver) = mpsc::channel(1);
    assert_eq!(
        mock.provider(ProviderKind::OpenAi, control.clone())?
            .stream(&request, sink, &RequestOptions::default())
            .await,
        Err(AiError::Disabled)
    );
    assert!(mock.requests.try_recv().is_err());
    assert!(store.audit()?.is_empty());
    store.save("connection", policy(), false)?;
    let request = TextRequest {
        context: gated(&store, &mock, &control)?,
        ..request
    };
    let other = Mock::new(vec!["hang".into()]).await?;
    let (sink, _receiver) = mpsc::channel(1);
    assert_eq!(
        other
            .provider(ProviderKind::OpenAi, control)?
            .stream(&request, sink, &RequestOptions::default())
            .await,
        Err(AiError::Disabled)
    );
    assert!(store.audit()?.is_empty());
    Ok(())
}
#[tokio::test]
async fn policy_revocation_cancels_active_transport() -> TestResult {
    let store = SharingStore::memory(1000)?;
    store.save("connection", policy(), false)?;
    let control = enabled();
    let mut mock = Mock::new(vec!["hang".into()]).await?;
    let request = TextRequest {
        model: "model".into(),
        context: gated(&store, &mock, &control)?,
        max_output: 10,
    };
    let provider = mock.provider(ProviderKind::OpenAi, control)?;
    let (sink, _receiver) = mpsc::channel(1);
    let options = RequestOptions::default();
    let revoke = async {
        mock.requests.recv().await;
        store.save("connection", SharingPolicy::default(), false)
    };
    let (result, saved) = tokio::join!(provider.stream(&request, sink, &options), revoke);
    saved?;
    assert_eq!(result, Err(AiError::Disabled));
    assert_eq!(store.audit()?.len(), 1);
    Ok(())
}
#[test]
fn device_store_survives_restart_and_is_private_without_raw_context() -> TestResult {
    let directory = tempfile::tempdir_in(".")?;
    let path = directory.path().join("sharing.sqlite");
    let store = SharingStore::open(&path)?;
    store.save("connection", policy(), false)?;
    drop(store);
    let reopened = SharingStore::open(&path)?;
    assert_eq!(reopened.policy("connection")?, policy());
    assert!(reopened.audit()?.is_empty());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path)?.permissions().mode() & 0o777,
            0o600
        );
    }
    Ok(())
}
