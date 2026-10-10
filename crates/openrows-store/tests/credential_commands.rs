// SPDX-License-Identifier: Apache-2.0
//! Direct invocation, timeout, cancellation, redaction and imported trust gate.
use openrows_store::profile::{
    CommandCancellation, CommandTrust, CredentialCommand, CredentialError, PasswordStorage,
    ProfileId, exchange, run_credential,
};
use std::{
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn helper(dir: &Path) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let executable = dir.join(if cfg!(windows) {
        "helper.exe"
    } else {
        "helper"
    });
    let result = std::process::Command::new("rustc")
        .args([
            "--edition=2024",
            "tests/fixtures/credential_helper.rs",
            "-o",
        ])
        .arg(&executable)
        .output()?;
    if !result.status.success() {
        return Err("credential helper compilation failed".into());
    }
    Ok(executable.canonicalize()?)
}
fn command(program: &Path, args: &[&str]) -> CredentialCommand {
    CredentialCommand {
        program: program.into(),
        args: args.iter().map(|arg| (*arg).into()).collect(),
        timeout_seconds: 10,
    }
}
#[test]
fn literal_shell_metacharacters_and_closed_stdin() -> TestResult {
    let dir = tempfile::tempdir()?;
    let program = helper(dir.path())?;
    let trust = CommandTrust::new(&dir.path().join("trust.json"));
    let id = ProfileId::new();
    for args in [&["args", "; $(touch forbidden) $HOME"][..], &["stdin"][..]] {
        let invocation = command(&program, args);
        trust.trust(id, &invocation)?;
        let password = run_credential(&trust, id, &invocation, &CommandCancellation::default())?;
        assert_eq!(
            password.expose(),
            if args[0] == "args" { args[1] } else { "pw" }
        );
    }
    Ok(())
}
#[test]
fn deadline_cancellation_and_stderr_are_typed_and_scrubbed() -> TestResult {
    let dir = tempfile::tempdir()?;
    let program = helper(dir.path())?;
    let trust = Arc::new(CommandTrust::new(&dir.path().join("trust.json")));
    let id = ProfileId::new();
    let failed = command(&program, &["fail"]);
    trust.trust(id, &failed)?;
    let error = run_credential(&trust, id, &failed, &CommandCancellation::default()).unwrap_err();
    assert!(matches!(error, CredentialError::Failed(_)));
    let text = error.to_string();
    for secret in ["test-only", "pw", "extra", "more"] {
        assert!(!text.contains(secret), "diagnostic contains {secret}");
    }
    let marker = dir.path().join("started");
    let mut blocking = command(&program, &["block", marker.to_str().ok_or("path")?]);
    blocking.timeout_seconds = 1;
    trust.trust(id, &blocking)?;
    assert!(matches!(
        run_credential(&trust, id, &blocking, &CommandCancellation::default()),
        Err(CredentialError::Timeout)
    ));
    std::fs::remove_file(&marker)?;
    blocking.timeout_seconds = 120;
    let cancel = CommandCancellation::default();
    let worker_cancel = cancel.clone();
    let worker_trust = Arc::clone(&trust);
    let worker =
        std::thread::spawn(move || run_credential(&worker_trust, id, &blocking, &worker_cancel));
    let started = Instant::now();
    while !marker.exists() && started.elapsed() < Duration::from_secs(5) {
        std::thread::yield_now();
    }
    assert!(marker.exists());
    cancel.cancel();
    assert!(matches!(
        worker.join().map_err(|_| "worker panic")?,
        Err(CredentialError::Cancelled)
    ));
    Ok(())
}
#[test]
fn imported_command_cannot_execute_and_edited_arguments_invalidate_trust() -> TestResult {
    let dir = tempfile::tempdir()?;
    let program = helper(dir.path())?;
    let marker = dir.path().join("started");
    let mut profile = openrows_store::profile::ConnectionProfile::new_postgres(
        "import",
        openrows_store::profile::ServerParams::default(),
        openrows_store::profile::EnvTag::new(openrows_store::profile::EnvKind::Local),
    );
    profile.password_storage = PasswordStorage::Command;
    profile.credential_command = Some(command(
        &program,
        &["block", marker.to_str().ok_or("path")?],
    ));
    let trust = CommandTrust::new(&dir.path().join("trusted-commands.json"));
    trust.trust(
        profile.id,
        profile.credential_command.as_ref().ok_or("command")?,
    )?;
    let batch = exchange::parse(&exchange::export(&[profile])?)?;
    let imported = &batch.entries[0].profile;
    let invocation = imported.credential_command.as_ref().ok_or("command")?;
    assert!(matches!(
        run_credential(
            &trust,
            imported.id,
            invocation,
            &CommandCancellation::default()
        ),
        Err(CredentialError::Untrusted)
    ));
    assert!(!marker.exists());
    trust.trust(imported.id, invocation)?;
    let mut changed = invocation.clone();
    changed.args.push("literal".into());
    assert!(!trust.is_trusted(imported.id, &changed)?);
    changed = invocation.clone();
    changed.program = dir.path().join("other");
    assert!(!trust.is_trusted(imported.id, &changed)?);
    assert!(
        CommandTrust::new(&dir.path().join("trusted-commands.json"))
            .is_trusted(imported.id, invocation)?
    );
    Ok(())
}

#[test]
fn libsql_command_token_uses_the_existing_trust_policy() -> TestResult {
    use openrows_store::profile::{EnvKind, EnvTag, parse_url};
    let dir = tempfile::tempdir_in(".")?;
    let program = helper(dir.path())?;
    let (mut profile, _) =
        parse_url("libsql://db.test")?.into_profile("remote", EnvTag::new(EnvKind::Local));
    profile.password_storage = PasswordStorage::Command;
    profile.credential_command = Some(command(&program, &["stdin"]));
    profile.validate()?;
    let invocation = profile.credential_command.as_ref().ok_or("command")?;
    let trust = CommandTrust::new(&dir.path().join("trust.json"));
    assert!(matches!(
        run_credential(
            &trust,
            profile.id,
            invocation,
            &CommandCancellation::default()
        ),
        Err(CredentialError::Untrusted)
    ));
    trust.trust(profile.id, invocation)?;
    let token = run_credential(
        &trust,
        profile.id,
        invocation,
        &CommandCancellation::default(),
    )?;
    assert_eq!(token.expose(), "pw");
    assert!(!exchange::export(&[profile])?.contains("\"pw\""));
    Ok(())
}
