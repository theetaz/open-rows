// SPDX-License-Identifier: Apache-2.0
//! Credential commands on reconnect: new output every time, side by side after
//! a wake, and never a dialog in the background (docs/12 §10).
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

use super::*;
use crate::connections::resilient::OneTimePassword;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// A provider that waits until `waits_for` runs are alive at once, so it only
/// finishes if they really overlap, and prints an answer unique to each run.
fn provider(dir: &Path, waits_for: usize) -> Result<CredentialCommand, io::Error> {
    let calls = dir.join("calls");
    let script = format!(
        "#!/bin/sh\necho x >> '{0}'\nuntil [ \"$(/usr/bin/wc -l < '{0}')\" -ge {waits_for} ]; do :; done\necho \"fresh-$$\"\n",
        calls.display()
    );
    let program = dir.join("provider");
    std::fs::write(&program, script)?;
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755))?;
    Ok(CredentialCommand {
        program,
        args: Vec::new(),
        timeout_seconds: 10,
    })
}

fn trusted(
    dir: &Path,
    id: ProfileId,
    command: &CredentialCommand,
) -> Result<Arc<CommandTrust>, IpcError> {
    let trust = Arc::new(CommandTrust::new(&dir.join("trusted-commands.json")));
    trust.trust(id, command)?;
    Ok(trust)
}

/// A session's password source, as `open_session` builds it.
fn session_password(
    runs: &Arc<CredentialRuns>,
    trust: &Arc<CommandTrust>,
    invocation: &(ProfileId, CredentialCommand),
) -> OneTimePassword {
    let (runs, trust, invocation) = (Arc::clone(runs), Arc::clone(trust), invocation.clone());
    OneTimePassword::new(
        Some("opening".into()),
        Arc::new(move || {
            let (runs, trust, invocation) =
                (Arc::clone(&runs), Arc::clone(&trust), invocation.clone());
            Box::pin(async move { reconnect_password(&runs, trust, invocation).await })
        }),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sessions_reconnecting_at_once_each_get_new_output() -> TestResult {
    const SESSIONS: usize = 4;
    let dir = tempfile::tempdir()?;
    let id = ProfileId::new();
    let command = provider(dir.path(), SESSIONS)?;
    let trust = trusted(dir.path(), id, &command)?;
    let runs = Arc::new(CredentialRuns::default());
    let mut woken = tokio::task::JoinSet::new();
    for _ in 0..SESSIONS {
        let password = session_password(&runs, &trust, &(id, command.clone()));
        assert_eq!(password.next().await?.expose(), "opening");
        woken.spawn(async move { password.next().await });
    }
    let mut seen = std::collections::HashSet::new();
    while let Some(reconnected) = woken.join_next().await {
        let password = reconnected??;
        assert!(password.expose().starts_with("fresh-"));
        seen.insert(password.expose().to_owned());
    }
    assert_eq!(seen.len(), SESSIONS, "no session shares or replays a value");
    runs.start(id)?;
    Ok(())
}

#[tokio::test]
async fn a_withdrawn_approval_fails_without_running_or_asking() -> TestResult {
    let dir = tempfile::tempdir()?;
    let command = provider(dir.path(), 1)?;
    let trust = Arc::new(CommandTrust::new(&dir.path().join("trusted-commands.json")));
    let runs = CredentialRuns::default();
    let failed = reconnect_password(&runs, trust, (ProfileId::new(), command)).await;
    assert!(matches!(failed, Err(DriverError::Auth(message)) if message.contains("Trust and run")));
    assert!(!dir.path().join("calls").exists(), "the program never ran");
    Ok(())
}

#[test]
fn reconnects_retry_only_what_can_pass_by_itself() {
    let failed = CredentialError::Failed("exit 1".to_owned());
    assert!(matches!(
        reconnect_failure(&failed),
        DriverError::Network(_)
    ));
    assert!(matches!(
        reconnect_failure(&CredentialError::Timeout),
        DriverError::Network(_)
    ));
    assert!(matches!(
        reconnect_failure(&CredentialError::Untrusted),
        DriverError::Auth(_)
    ));
    assert!(matches!(
        reconnect_failure(&CredentialError::Cancelled),
        DriverError::Auth(_)
    ));
}

#[test]
fn a_trust_question_stays_alone_and_a_dropped_reconnect_leaves_no_attempt() -> Result<(), IpcError>
{
    let runs = CredentialRuns::default();
    let id = ProfileId::new();
    let joined = Joined::new(&runs, id)?;
    assert!(
        runs.start(id).is_err(),
        "a question waits for running attempts"
    );
    assert!(runs.join(id).is_ok(), "reconnects run side by side");
    runs.finish(id);
    drop(joined);
    runs.start(id)?;
    runs.finish(id);
    Ok(())
}

#[test]
fn a_connect_with_an_approved_command_runs_beside_reconnects() -> TestResult {
    let dir = tempfile::tempdir()?;
    let id = ProfileId::new();
    let command = provider(dir.path(), 1)?;
    let runs = CredentialRuns::default();
    let unapproved = CommandTrust::new(&dir.path().join("other.json"));
    begin(&runs, &unapproved, id, &command)?;
    assert!(
        begin(&runs, &unapproved, id, &command).is_err(),
        "one trust question at a time"
    );
    runs.finish(id);
    let approved = trusted(dir.path(), id, &command)?;
    begin(&runs, &approved, id, &command)?;
    begin(&runs, &approved, id, &command)?;
    Ok(())
}
