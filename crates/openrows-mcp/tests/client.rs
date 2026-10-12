// SPDX-License-Identifier: Apache-2.0
//! Both official client transports and the fail-closed consent/redaction boundary.
mod support;
use openrows_ai::{Cancellation, sharing::RedactionRule};
use openrows_mcp::client::{
    CallInput, ClientError, ClientTool, Consent, Credentials, Endpoint, ServerConfig, call,
    discover, redact_json,
};
use serde_json::json;
use support::{Fixture, TestResult};
fn config(endpoint: Endpoint) -> ServerConfig {
    ServerConfig {
        id: "fixture".into(),
        name: "Fixture".into(),
        endpoint,
        enabled_tools: vec!["list_connections".into()],
    }
}
fn tool() -> ClientTool {
    ClientTool {
        name: "list_connections".into(),
        schema: json!({"type":"object"}),
    }
}
#[test]
fn consent_is_required_per_server_tool_and_schema_and_revoked_on_change() -> TestResult {
    let config = config(Endpoint::Http {
        url: "http://127.0.0.1:1/mcp".into(),
    });
    let mut consent = Consent::default();
    let tool = tool();
    assert_eq!(
        consent.check(&config, &tool),
        Err(ClientError::Confirmation)
    );
    consent.confirm(&config, &tool)?;
    assert_eq!(consent.check(&config, &tool), Ok(()));
    let changed = ClientTool {
        schema: json!({"type":"object","required":["other"]}),
        ..tool.clone()
    };
    assert_eq!(
        consent.check(&config, &changed),
        Err(ClientError::Confirmation)
    );
    let other = ServerConfig {
        id: "second".into(),
        ..config.clone()
    };
    assert_eq!(consent.check(&other, &tool), Err(ClientError::Confirmation));
    let disabled = ServerConfig {
        enabled_tools: vec![],
        ..config.clone()
    };
    assert_eq!(consent.check(&disabled, &tool), Err(ClientError::Disabled));
    assert_eq!(
        consent.confirm(&disabled, &tool),
        Err(ClientError::Disabled)
    );
    consent.clear();
    assert_eq!(
        consent.check(&config, &tool),
        Err(ClientError::Confirmation)
    );
    Ok(())
}
#[tokio::test]
async fn disabled_unconfirmed_and_cancelled_calls_never_launch_transport() -> TestResult {
    let config = config(Endpoint::Stdio {
        command: "nonexistent-openrows-fixture".into(),
        args: vec![],
    });
    let secrets = Credentials::default();
    let mut consent = Consent::default();
    let tool = tool();
    let input = || CallInput {
        tool: &tool,
        arguments: json!({}),
        consent: &consent,
    };
    assert!(matches!(
        call(&config, &secrets, input(), &Cancellation::default()).await,
        Err(ClientError::Confirmation)
    ));
    let disabled = ServerConfig {
        enabled_tools: vec![],
        ..config.clone()
    };
    assert!(matches!(
        call(&disabled, &secrets, input(), &Cancellation::default()).await,
        Err(ClientError::Disabled)
    ));
    consent.confirm(&config, &tool)?;
    let cancel = Cancellation::default();
    cancel.cancel();
    assert!(matches!(
        call(
            &config,
            &secrets,
            CallInput {
                tool: &tool,
                arguments: json!({}),
                consent: &consent
            },
            &cancel
        )
        .await,
        Err(ClientError::Cancelled)
    ));
    Ok(())
}
#[test]
fn nested_values_credentials_sql_literals_and_policy_rules_are_redacted() -> TestResult {
    let mut secrets = Credentials::default();
    secrets
        .env
        .insert("FIXTURE_KEY".into(), "synthetic-value".into());
    secrets
        .headers
        .insert("Authorization".into(), "Bearer synthetic-header".into());
    let value = json!({"password":"never-visible", "nested":[{"api_key":"never-visible", "text":"synthetic-value synthetic-header", "sql":"SELECT 'private-literal' FROM public.items", "private_column":"never-visible"}]});
    let output = redact_json(
        &value,
        &secrets,
        openrows_sql::Dialect::Postgres,
        &[RedactionRule::Glob("private_*".into())],
    )?;
    let text = serde_json::to_string(&output)?;
    for forbidden in [
        "never-visible",
        "synthetic-value",
        "synthetic-header",
        "private-literal",
        "private_column",
    ] {
        assert!(!text.contains(forbidden), "{forbidden}: {text}");
    }
    assert!(text.contains("redacted"));
    assert!(!format!("{secrets:?}").contains("synthetic"));
    assert!(!ClientError::Transport.to_string().contains("synthetic"));
    Ok(())
}
#[test]
fn configuration_rejects_credentials_protocol_headers_and_remote_plaintext() {
    for url in [
        "https://user:pass@example.com/mcp",
        "https://example.com/mcp?key=value",
        "http://example.com/mcp",
        "https://example.com/mcp#secret",
    ] {
        assert!(
            config(Endpoint::Http { url: url.into() })
                .validate()
                .is_err()
        );
    }
    assert!(serde_json::from_value::<ServerConfig>(json!({"id":"fixture","name":"Fixture","endpoint":{"kind":"http","url":"https://example.com/mcp"},"enabledTools":[],"headers":{"Authorization":"test-only"}})).is_err());
    let mut secrets = Credentials::default();
    secrets
        .headers
        .insert("mcp-session-id".into(), "test-only".into());
    assert_eq!(secrets.validate(), Err(ClientError::Invalid));
}
#[tokio::test]
async fn http_discovery_confirmation_and_call_round_trip() -> TestResult {
    let fixture = Fixture::new(true).await?;
    let config = config(Endpoint::Http {
        url: format!("http://127.0.0.1:{}/mcp", fixture.server.port()),
    });
    let mut secrets = Credentials::default();
    secrets.headers.insert(
        "Authorization".into(),
        format!("Bearer {}", fixture.token.value),
    );
    round_trip(&config, &secrets).await
}
#[tokio::test]
async fn stdio_discovery_confirmation_and_call_round_trip() -> TestResult {
    let fixture = Fixture::new(false).await?;
    let config = config(Endpoint::Stdio {
        command: env!("CARGO_BIN_EXE_openrows-mcp").into(),
        args: vec!["--port".into(), fixture.server.port().to_string()],
    });
    let mut secrets = Credentials::default();
    secrets
        .env
        .insert("OPENROWS_MCP_TOKEN".into(), fixture.token.value.clone());
    round_trip(&config, &secrets).await
}
async fn round_trip(config: &ServerConfig, secrets: &Credentials) -> TestResult {
    let cancel = Cancellation::default();
    let tools = discover(config, secrets, &cancel).await?;
    assert_eq!(tools.len(), 7);
    assert!(
        !tools
            .iter()
            .any(|tool| tool.name.contains("write") || tool.name.contains("execute"))
    );
    let tool = tools
        .iter()
        .find(|tool| tool.name == "list_connections")
        .ok_or("tool missing")?;
    let mut consent = Consent::default();
    consent.confirm(config, tool)?;
    let result = call(
        config,
        secrets,
        CallInput {
            tool,
            arguments: json!({}),
            consent: &consent,
        },
        &cancel,
    )
    .await?;
    assert_ne!(result.is_error, Some(true));
    assert!(serde_json::to_string(&result)?.contains("connection"));
    Ok(())
}

#[tokio::test]
async fn tool_error_results_are_returned_for_policy_filtered_trace() -> TestResult {
    let fixture = Fixture::new(true).await?;
    let mut config = config(Endpoint::Http {
        url: format!("http://127.0.0.1:{}/mcp", fixture.server.port()),
    });
    config.enabled_tools = vec!["list_schemas".into()];
    let mut secrets = Credentials::default();
    secrets.headers.insert(
        "Authorization".into(),
        format!("Bearer {}", fixture.token.value),
    );
    let tools = discover(&config, &secrets, &Cancellation::default()).await?;
    let tool = tools
        .iter()
        .find(|tool| tool.name == "list_schemas")
        .ok_or("missing tool")?;
    let mut consent = Consent::default();
    consent.confirm(&config, tool)?;
    let result = call(
        &config,
        &secrets,
        CallInput {
            tool,
            arguments: json!({}),
            consent: &consent,
        },
        &Cancellation::default(),
    )
    .await?;
    assert_eq!(result.is_error, Some(true));
    assert!(serde_json::to_string(&result)?.contains("Invalid MCP request"));
    Ok(())
}
