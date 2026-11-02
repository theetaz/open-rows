// SPDX-License-Identifier: Apache-2.0
use super::*;
use openrows_mcp::client::{ClientTool, Endpoint};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn search() -> ProjectServer {
    ProjectServer {
        id: "search".into(),
        name: "Search".into(),
        endpoint: Endpoint::Stdio {
            command: "/usr/local/bin/search-mcp".into(),
            args: vec!["--read-only".into()],
        },
        env: vec!["SEARCH_TOKEN".into()],
        headers: vec![],
    }
}
fn tool() -> ClientTool {
    ClientTool {
        name: "lookup".into(),
        schema: serde_json::json!({"type":"object"}),
    }
}
fn state(dir: &tempfile::TempDir) -> Result<State, IpcError> {
    State::new(dir.path().join("mcp-servers.json"))
}
fn key() -> String {
    project_key("/work/app", "search")
}
/// Trust, discover and enable `lookup`, as the user would.
fn trusted(state: &State) -> TestResult {
    state.register_project("/work/app", "App", &[search()]);
    state.trust_project(&key(), &search().digest())?;
    let config = state.loaded(&key())?.config().ok_or("not trusted")?;
    let revision = state.lock().revision;
    state.store_tools(&config, revision, vec![tool()])?;
    state.enable_project_tools(&key(), vec!["lookup".into()])?;
    Ok(())
}

#[test]
fn untrusted_project_servers_are_listed_but_never_runnable() -> TestResult {
    let dir = tempfile::tempdir()?;
    let state = state(&dir)?;
    let deleted = state.register_project("/work/app", "App", &[search()]);
    assert!(deleted.is_empty());
    let inner = state.lock();
    assert_eq!(inner.projects.len(), 1);
    assert!(inner.servers().is_empty());
    assert!(super::super::aliases::enabled_tools(&inner).is_empty());
    assert!(inner.projects[0].config().is_none());
    Ok(())
}

#[test]
fn trust_needs_the_reviewed_digest_and_tools_start_disabled() -> TestResult {
    let dir = tempfile::tempdir()?;
    let state = state(&dir)?;
    state.register_project("/work/app", "App", &[search()]);
    assert!(state.trust_project(&key(), "stale").is_err());
    assert!(state.lock().servers().is_empty());
    state.trust_project(&key(), &search().digest())?;
    let servers = state.lock().servers();
    assert_eq!(servers.len(), 1);
    assert!(servers[0].enabled_tools.is_empty());
    assert_eq!(servers[0].name, "App: Search");
    assert!(state.enable_project_tools("unknown", vec![]).is_err());
    Ok(())
}

#[test]
fn trusted_enabled_tools_reach_the_assistant_and_survive_restart() -> TestResult {
    let dir = tempfile::tempdir()?;
    trusted(&state(&dir)?)?;
    let restarted = state(&dir)?;
    restarted.register_project("/work/app", "App", &[search()]);
    let inner = restarted.lock();
    let tools = super::super::aliases::enabled_tools(&inner);
    assert_eq!(tools.len(), 1);
    assert_eq!(
        super::super::aliases::tool_label(&tools, "user_0_0").as_deref(),
        Some("App: Search → lookup")
    );
    let catalogue = std::fs::read_to_string(dir.path().join("mcp-tools.json")).unwrap_or_default();
    assert!(
        !catalogue.contains("lookup"),
        "Project catalogue leaked into user file"
    );
    Ok(())
}

#[test]
fn a_changed_definition_revokes_trust_grants_catalogue_and_values() -> TestResult {
    let dir = tempfile::tempdir()?;
    let state = state(&dir)?;
    trusted(&state)?;
    let config = state.loaded(&key())?.config().ok_or("not trusted")?;
    state.lock().consent.confirm(&config, &tool())?;
    let revision = state.lock().revision;
    let mut changed = search();
    changed.endpoint = Endpoint::Stdio {
        command: "/tmp/other-mcp".into(),
        args: vec![],
    };
    let deleted = state.register_project("/work/app", "App", &[changed.clone()]);
    assert_eq!(deleted, [key()]);
    let inner = state.lock();
    assert!(inner.servers().is_empty());
    assert!(inner.consent.check(&config, &tool()).is_err());
    assert_ne!(inner.revision, revision);
    assert!(!inner.tools.contains_key(&key()));
    assert!(inner.projects[0].changed);
    drop(inner);
    // Reloading keeps the change visible; trusting the new definition clears it.
    state.register_project("/work/app", "App", &[changed.clone()]);
    assert!(state.loaded(&key())?.changed);
    state.trust_project(&key(), &changed.digest())?;
    let entry = state.loaded(&key())?;
    assert!(
        !entry.changed
            && entry
                .trust
                .is_some_and(|trust| trust.enabled_tools.is_empty())
    );
    Ok(())
}

#[test]
fn revoking_and_unloading_stop_offering_the_server() -> TestResult {
    let dir = tempfile::tempdir()?;
    let state = state(&dir)?;
    trusted(&state)?;
    state.revoke_project(&key())?;
    assert!(state.lock().servers().is_empty());
    trusted(&state)?;
    state.register_project("/work/app", "App", &[]);
    assert!(state.lock().servers().is_empty());
    assert!(state.loaded(&key()).is_err());
    Ok(())
}

#[test]
fn only_declared_names_can_hold_values() -> TestResult {
    let mut current = Credentials::default();
    current.env.insert("LEFTOVER".into(), "test-only".into());
    let next = merge_secret(&search(), current, "SEARCH_TOKEN", "test-only".into())?;
    assert_eq!(next.env.keys().collect::<Vec<_>>(), ["SEARCH_TOKEN"]);
    assert!(
        merge_secret(
            &search(),
            Credentials::default(),
            "PATH",
            "test-only".into()
        )
        .is_err()
    );
    Ok(())
}
