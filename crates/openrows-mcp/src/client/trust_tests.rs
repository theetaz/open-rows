// SPDX-License-Identifier: Apache-2.0
use super::super::project::tests::stdio;
use super::*;

fn store() -> Result<(tempfile::TempDir, ProjectTrust), std::io::Error> {
    let dir = tempfile::tempdir()?;
    let trust = ProjectTrust::new(&dir.path().join("mcp-project-trust.json"));
    Ok((dir, trust))
}

#[test]
fn servers_start_untrusted_and_trust_needs_the_reviewed_digest()
-> Result<(), Box<dyn std::error::Error>> {
    let (_dir, trust) = store()?;
    let server = stdio();
    assert!(matches!(trust.state("k", &server)?, TrustState::Untrusted));
    assert!(trust.trust("k", &server, "stale-digest").is_err());
    assert!(matches!(trust.state("k", &server)?, TrustState::Untrusted));
    trust.trust("k", &server, &server.digest())?;
    assert!(matches!(trust.state("k", &server)?, TrustState::Trusted(_)));
    assert!(matches!(
        trust.state("other", &server)?,
        TrustState::Untrusted
    ));
    Ok(())
}

#[test]
fn a_changed_definition_revokes_trust_tools_and_catalogue() -> Result<(), Box<dyn std::error::Error>>
{
    let (_dir, trust) = store()?;
    let server = stdio();
    trust.trust("k", &server, &server.digest())?;
    trust.edit("k", &server, |entry| {
        entry.enabled_tools = vec!["search".into()];
        entry.tools = vec![ClientTool {
            name: "search".into(),
            schema: serde_json::json!({"type":"object"}),
        }];
    })?;
    let mut changed = server.clone();
    changed.endpoint = Endpoint::Stdio {
        command: "/tmp/other".into(),
        args: vec![],
    };
    assert!(trust.edit("k", &changed, |_| {}).is_err());
    assert!(matches!(trust.state("k", &changed)?, TrustState::Changed));
    assert!(matches!(trust.state("k", &server)?, TrustState::Untrusted));
    Ok(())
}

#[test]
fn revoke_forgets_and_the_file_is_private() -> Result<(), Box<dyn std::error::Error>> {
    let (dir, trust) = store()?;
    let server = stdio();
    trust.trust("k", &server, &server.digest())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(dir.path().join("mcp-project-trust.json"))?
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    trust.revoke("k")?;
    assert!(matches!(trust.state("k", &server)?, TrustState::Untrusted));
    assert!(trust.edit("k", &server, |_| {}).is_err());
    Ok(())
}

use super::super::Endpoint;
