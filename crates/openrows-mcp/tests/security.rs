// SPDX-License-Identifier: Apache-2.0
//! security regression fixtures.
mod support;
use openrows_ai::sharing::SharingLevel;
use openrows_mcp::{McpSettings, TokenStore};
use serde_json::json;
use std::sync::atomic::Ordering;
use support::{Fixture, TestResult, args, message, policy, post, scope};

#[tokio::test]
async fn tokens_default_off_http_origin_scope_and_hash_only_storage() -> TestResult {
    assert!(!McpSettings::default().enabled && !McpSettings::default().http_enabled);
    let fixture = Fixture::new(false).await?;
    let list = message("tools/list", json!({}));
    assert_eq!(
        post(
            fixture.server.port(),
            "/mcp",
            &fixture.token.value,
            list.clone(),
            false
        )
        .await?
        .0,
        404
    );
    assert_eq!(
        post(
            fixture.server.port(),
            "/bridge",
            &fixture.token.value,
            list.clone(),
            true
        )
        .await?
        .0,
        403
    );
    assert_eq!(
        post(fixture.server.port(), "/bridge", "test-only", list, false)
            .await?
            .0,
        401
    );
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("tokens.sqlite");
    let tokens = TokenStore::open(&path)?;
    let created = tokens.create(scope())?;
    assert!(!serde_json::to_string(&tokens.list()?)?.contains(&created.value));
    let bytes = std::fs::read(&path)?;
    assert!(
        !bytes
            .windows(created.value.len())
            .any(|window| window == created.value.as_bytes())
    );
    let mut expired = scope();
    expired.expires = 1;
    assert!(tokens.create(expired).is_err());
    tokens.revoke(&created.info.id)?;
    assert!(tokens.list()?.is_empty());
    Ok(())
}
#[tokio::test]
async fn sharing_levels_allowlist_caps_revocation_and_proposals() -> TestResult {
    let fixture = Fixture::new(true).await?;
    fixture
        .host
        .sharing
        .save("connection", policy(SharingLevel::SchemaOnly), false)?;
    assert_eq!(
        fixture.call("run_read_query", args()).await?["result"]["isError"],
        true
    );
    fixture
        .host
        .sharing
        .save("connection", policy(SharingLevel::None), false)?;
    let (_, list) = post(
        fixture.server.port(),
        "/mcp",
        &fixture.token.value,
        message(
            "tools/call",
            json!({"name":"list_connections","arguments":{}}),
        ),
        false,
    )
    .await?;
    assert!(
        list["result"]["content"][0]["text"]
            .as_str()
            .ok_or("missing")?
            .contains("\"connections\":[]")
    );
    let mut denied = policy(SharingLevel::SampleRows);
    denied.providers.clear();
    fixture.host.sharing.save("connection", denied, false)?;
    assert_eq!(
        fixture.call("run_read_query", args()).await?["result"]["isError"],
        true
    );
    fixture
        .host
        .sharing
        .save("connection", policy(SharingLevel::SampleRows), false)?;
    let outside = fixture
        .call(
            "propose_sql",
            json!({"connection":"outside","database":"main","sql":"DELETE FROM items"}),
        )
        .await?;
    assert_eq!(outside["result"]["isError"], true);
    assert_eq!(fixture.backend.proposals.load(Ordering::SeqCst), 0);
    let proposal = fixture
        .call(
            "propose_sql",
            json!({"connection":"connection","database":"main","sql":"DELETE FROM items"}),
        )
        .await?;
    assert_ne!(proposal["result"]["isError"], true);
    assert_eq!(fixture.backend.proposals.load(Ordering::SeqCst), 1);
    Ok(())
}
#[tokio::test]
async fn revoke_disconnect_ai_off_sharing_change_and_close_cancel_in_flight() -> TestResult {
    for action in 0..5 {
        let fixture = Fixture::new(true).await?;
        fixture.backend.wait.store(true, Ordering::SeqCst);
        let port = fixture.server.port();
        let credential = fixture.token.value.clone();
        let task = tokio::spawn(async move {
            post(
                port,
                "/mcp",
                &credential,
                message(
                    "tools/call",
                    json!({"name":"run_read_query","arguments":args()}),
                ),
                false,
            )
            .await
            .map_err(|error| error.to_string())
        });
        fixture.backend.entered.notified().await;
        match action {
            0 => fixture.host.tokens.revoke(&fixture.token.info.id)?,
            1 => fixture.host.tokens.disconnect(&fixture.token.info.id),
            2 => fixture.host.control.set_enabled(false),
            3 => fixture
                .host
                .sharing
                .save("connection", policy(SharingLevel::None), false)?,
            _ => {
                fixture.backend.open.store(false, Ordering::SeqCst);
                fixture.host.tokens.cancel_all();
            }
        }
        let (_, response) = tokio::time::timeout(std::time::Duration::from_secs(2), task)
            .await??
            .map_err(std::io::Error::other)?;
        assert_eq!(response["result"]["isError"], true, "{response}");
        assert!(fixture.host.sharing.audit()?.is_empty());
    }
    Ok(())
}

#[tokio::test]
async fn output_caps_production_local_only_and_plan_permission_fail_closed() -> TestResult {
    let fixture = Fixture::new(true).await?;
    fixture.backend.large.store(true, Ordering::SeqCst);
    let response = fixture.call("run_read_query", args()).await?;
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .ok_or("missing payload")?;
    assert!(text.len() <= 16384);
    let bundle: serde_json::Value = serde_json::from_str(text)?;
    let rows = bundle["samples"][0]["rows"]
        .as_array()
        .ok_or("missing rows")?;
    assert!(rows.len() <= 20);
    assert!(
        rows.iter()
            .map(|row| row.as_array().map_or(0, Vec::len))
            .sum::<usize>()
            <= 200
    );
    assert!(!text.contains("test-only"));
    fixture.backend.production.store(true, Ordering::SeqCst);
    assert_eq!(
        fixture.call("run_read_query", args()).await?["result"]["isError"],
        true
    );
    fixture.backend.production.store(false, Ordering::SeqCst);
    let mut denied = policy(SharingLevel::SampleRows);
    denied.local_only = true;
    fixture.host.sharing.save("connection", denied, false)?;
    assert_eq!(
        fixture.call("run_read_query", args()).await?["result"]["isError"],
        true
    );
    let mut denied = policy(SharingLevel::SchemaAndStats);
    denied.telemetry = false;
    fixture.host.sharing.save("connection", denied, false)?;
    assert_eq!(
        fixture.call("explain_query", args()).await?["result"]["isError"],
        true
    );
    assert_eq!(fixture.host.sharing.audit()?.len(), 1);
    Ok(())
}

#[tokio::test]
async fn unfinished_reads_hit_the_tool_deadline_without_disclosure() -> TestResult {
    let fixture = Fixture::new(true).await?;
    fixture.backend.wait.store(true, Ordering::SeqCst);
    let response = tokio::time::timeout(
        std::time::Duration::from_secs(6),
        fixture.call("run_read_query", args()),
    )
    .await??;
    assert_eq!(response["result"]["isError"], true);
    assert!(fixture.host.sharing.audit()?.is_empty());
    Ok(())
}

#[tokio::test]
async fn connection_listing_cannot_release_an_earlier_revoked_disclosure() -> TestResult {
    let fixture = Fixture::new(true).await?;
    fixture
        .host
        .sharing
        .save("second", policy(SharingLevel::SampleRows), false)?;
    let mut scope = scope();
    scope.connections.push("second".into());
    let token = fixture.host.tokens.create(scope)?;
    *fixture
        .backend
        .revoke_on_second
        .lock()
        .map_err(|_| "poisoned")? = Some(fixture.host.sharing.clone());
    let (_, response) = post(
        fixture.server.port(),
        "/mcp",
        &token.value,
        message(
            "tools/call",
            json!({"name":"list_connections", "arguments":{}}),
        ),
        false,
    )
    .await?;
    assert_eq!(response["result"]["isError"], true);
    assert!(fixture.host.sharing.audit()?.is_empty());
    Ok(())
}
