// SPDX-License-Identifier: Apache-2.0
use super::*;

pub(crate) fn stdio() -> ProjectServer {
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

#[test]
fn a_declared_server_carries_names_but_never_values() -> Result<(), serde_json::Error> {
    let server: ProjectServer = serde_json::from_str(
        r#"{"id":"docs","name":"Docs","endpoint":{"kind":"http","url":"https://mcp.example.test/mcp"},"headers":["Authorization"]}"#,
    )?;
    assert!(server.validate().is_ok());
    assert!(server.declares("Authorization"));
    for secret in [
        r#"{"id":"d","name":"D","endpoint":{"kind":"http","url":"https://mcp.example.test/"},"headers":{"Authorization":"test-only"}}"#,
        r#"{"id":"s","name":"S","endpoint":{"kind":"stdio","command":"x","args":[]},"env":{"TOKEN":"test-only"}}"#,
        r#"{"id":"s","name":"S","endpoint":{"kind":"stdio","command":"x","args":[]},"credentials":{"TOKEN":"test-only"}}"#,
    ] {
        assert!(
            serde_json::from_str::<ProjectServer>(secret).is_err(),
            "{secret}"
        );
    }
    Ok(())
}

#[test]
fn invalid_definitions_are_refused() {
    let mut cases = Vec::new();
    let mut server = stdio();
    server.headers = vec!["Authorization".into()];
    cases.push(server);
    let mut server = stdio();
    server.env = vec!["1BAD".into()];
    cases.push(server);
    let mut server = stdio();
    server.env = vec!["A".into(), "A".into()];
    cases.push(server);
    let mut server = stdio();
    server.id = "x".repeat(49);
    cases.push(server);
    let mut server = stdio();
    server.endpoint = Endpoint::Http {
        url: "https://user:pass@mcp.example.test/".into(),
    };
    server.env.clear();
    cases.push(server);
    let mut server = stdio();
    server.endpoint = Endpoint::Http {
        url: "https://mcp.example.test/?token=x".into(),
    };
    server.env.clear();
    cases.push(server);
    let mut server = stdio();
    server.endpoint = Endpoint::Http {
        url: "https://mcp.example.test/".into(),
    };
    server.env.clear();
    server.headers = vec!["Mcp-Session-Id".into()];
    cases.push(server);
    for server in cases {
        assert!(server.validate().is_err(), "{}", server.id);
    }
    assert!(validate_project_servers(&[stdio(), stdio()]).is_err());
    assert!(validate_project_servers(&vec![stdio(); PROJECT_SERVER_LIMIT + 1]).is_err());
    assert!(validate_project_servers(&[stdio()]).is_ok());
}

#[test]
fn the_digest_covers_every_field_and_keys_are_per_project() {
    let base = stdio();
    let mut changed = Vec::new();
    let mut server = base.clone();
    server.endpoint = Endpoint::Stdio {
        command: "/usr/local/bin/search-mcp".into(),
        args: vec!["--write".into()],
    };
    changed.push(server);
    let mut server = base.clone();
    server.endpoint = Endpoint::Stdio {
        command: "/tmp/search-mcp".into(),
        args: vec!["--read-only".into()],
    };
    changed.push(server);
    let mut server = base.clone();
    server.env.push("OTHER".into());
    changed.push(server);
    let mut server = base.clone();
    server.name = "Renamed".into();
    changed.push(server);
    for server in changed {
        assert_ne!(server.digest(), base.digest());
    }
    assert_eq!(base.digest(), stdio().digest());
    let first = project_key("/work/a", "search");
    assert_ne!(first, project_key("/work/b", "search"));
    assert!(openrows_ai::validate_id(&first).is_ok());
    assert!(openrows_ai::validate_id(&project_key("/work/a", &"x".repeat(48))).is_ok());
}

#[test]
fn only_declared_credentials_reach_the_server() {
    let mut credentials = Credentials::default();
    credentials
        .env
        .insert("SEARCH_TOKEN".into(), "test-only".into());
    credentials
        .env
        .insert("PATH_HIJACK".into(), "test-only".into());
    credentials
        .headers
        .insert("Authorization".into(), "test-only".into());
    let kept = stdio().restrict(credentials);
    assert_eq!(kept.env.keys().collect::<Vec<_>>(), ["SEARCH_TOKEN"]);
    assert!(kept.headers.is_empty());
}
