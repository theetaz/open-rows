// SPDX-License-Identifier: Apache-2.0
use super::*;
fn config() -> ServerConfig {
    ServerConfig {
        id: "fixture".into(),
        name: "Fixture".into(),
        endpoint: openrows_mcp::client::Endpoint::Http {
            url: "http://127.0.0.1:1/mcp".into(),
        },
        enabled_tools: vec!["lookup".into()],
    }
}
#[test]
fn confirmation_responses_require_a_live_pending_request() -> Result<(), Box<dyn std::error::Error>>
{
    let dir = tempfile::tempdir()?;
    let state = State::new(dir.path().join("mcp-servers.json"))?;
    assert!(state.respond("unsolicited", true).is_err());
    for allow in [false, true] {
        let (send, receive) = oneshot::channel();
        state.lock().pending.insert("pending".into(), send);
        state.respond("pending", allow)?;
        assert_eq!(receive.blocking_recv()?, allow);
        assert!(state.respond("pending", true).is_err());
    }
    let (send, receive) = oneshot::channel();
    state.lock().pending.insert("expired".into(), send);
    drop(receive);
    assert!(state.respond("expired", true).is_err());
    assert!(state.lock().pending.is_empty());
    Ok(())
}
#[test]
fn public_metadata_round_trips_without_credential_fields() -> Result<(), Box<dyn std::error::Error>>
{
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("mcp-servers.json");
    let state = State::new(path.clone())?;
    state.save(vec![config()])?;
    let restored = State::new(path.clone())?;
    assert!(restored.lock().configs == vec![config()]);
    let text = std::fs::read_to_string(&path)?;
    assert!(!text.contains("headers") && !text.contains("env"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path)?.permissions().mode() & 0o777,
            0o600
        );
    }
    assert!(!path.with_extension("tmp").exists());
    Ok(())
}
#[test]
fn saving_disabled_tools_revokes_grants_and_drops_pending_prompts()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let state = State::new(dir.path().join("mcp-servers.json"))?;
    let saved = config();
    let tool = ClientTool {
        name: "lookup".into(),
        schema: serde_json::json!({"type":"object"}),
    };
    state.save(vec![saved.clone()])?;
    let (send, receive) = oneshot::channel();
    {
        let mut inner = state.lock();
        inner.consent.confirm(&saved, &tool)?;
        inner.pending.insert("pending".into(), send);
        inner.tools.insert(saved.id.clone(), vec![tool.clone()]);
    }
    let disabled = ServerConfig {
        enabled_tools: vec![],
        ..saved.clone()
    };
    state.save(vec![disabled.clone()])?;
    assert!(receive.blocking_recv().is_err());
    assert!(state.lock().consent.check(&saved, &tool).is_err());
    assert!(state.lock().consent.check(&disabled, &tool).is_err());
    assert_eq!(state.lock().tools[&saved.id].len(), 1);
    state.invalidate();
    assert!(state.lock().pending.is_empty());
    Ok(())
}
#[test]
fn changed_endpoints_discard_catalogues_and_invalid_config_never_writes()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("mcp-servers.json");
    let state = State::new(path.clone())?;
    let saved = config();
    state.save(vec![saved.clone()])?;
    state.lock().tools.insert(saved.id.clone(), vec![]);
    let changed = ServerConfig {
        endpoint: openrows_mcp::client::Endpoint::Http {
            url: "http://localhost:2/mcp".into(),
        },
        ..saved.clone()
    };
    state.save(vec![changed])?;
    assert!(state.lock().tools.is_empty());
    let before = std::fs::read(&path)?;
    assert!(state.save(vec![saved.clone(), saved]).is_err());
    assert_eq!(std::fs::read(path)?, before);
    Ok(())
}
#[test]
fn credential_changes_discard_schemas_filtered_with_previous_secrets()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let state = State::new(dir.path().join("mcp-servers.json"))?;
    let saved = config();
    let tool = ClientTool {
        name: "lookup".into(),
        schema: serde_json::json!({"type":"object"}),
    };
    state.save(vec![saved.clone()])?;
    state
        .lock()
        .tools
        .insert("other".into(), vec![tool.clone()]);
    // The second phase models discovery/consent during a keychain prompt.
    for _ in 0..2 {
        let (send, receive) = oneshot::channel();
        let revision = {
            let mut inner = state.lock();
            inner.consent.confirm(&saved, &tool)?;
            inner.tools.insert(saved.id.clone(), vec![tool.clone()]);
            inner.pending.insert("pending".into(), send);
            inner.revision
        };
        assert!(state.lock().current(revision, &saved));
        state.invalidate_credentials(&saved.id);
        assert!(!state.lock().current(revision, &saved));
        assert!(receive.blocking_recv().is_err());
        let inner = state.lock();
        assert_ne!(inner.revision, revision);
        assert!(!inner.tools.contains_key(&saved.id));
        assert!(inner.tools.contains_key("other"));
        assert!(inner.consent.check(&saved, &tool).is_err());
    }
    Ok(())
}
#[test]
fn catalogue_survives_restart_until_credentials_change() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("mcp-servers.json");
    let state = State::new(path.clone())?;
    let saved = config();
    let tool = ClientTool {
        name: "lookup".into(),
        schema: serde_json::json!({"type":"object"}),
    };
    state.save(vec![saved.clone()])?;
    let revision = state.lock().revision;
    state.store_tools(&saved, revision, vec![tool.clone()])?;
    let restored = State::new(path.clone())?;
    assert!(restored.lock().tools[&saved.id] == vec![tool.clone()]);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let catalogue = dir.path().join("mcp-tools.json");
        assert_eq!(
            std::fs::metadata(catalogue)?.permissions().mode() & 0o777,
            0o600
        );
    }
    let earlier = restored.lock().revision;
    restored.invalidate_credentials(&saved.id);
    assert!(restored.store_tools(&saved, earlier, vec![tool]).is_err());
    assert!(State::new(path)?.lock().tools.is_empty());
    Ok(())
}
#[test]
fn toggling_another_tool_keeps_existing_grants() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let state = State::new(dir.path().join("mcp-servers.json"))?;
    let saved = config();
    let tool = ClientTool {
        name: "lookup".into(),
        schema: serde_json::json!({"type":"object"}),
    };
    state.save(vec![saved.clone()])?;
    state.lock().consent.confirm(&saved, &tool)?;
    let toggled = ServerConfig {
        name: "Renamed".into(),
        enabled_tools: vec!["lookup".into(), "other".into()],
        ..saved
    };
    state.save(vec![toggled.clone()])?;
    assert!(state.lock().consent.check(&toggled, &tool).is_ok());
    state.invalidate();
    assert!(state.lock().consent.check(&toggled, &tool).is_err());
    Ok(())
}
