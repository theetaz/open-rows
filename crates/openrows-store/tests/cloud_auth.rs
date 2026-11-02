// SPDX-License-Identifier: Apache-2.0
//! Cloud tokens through fake CLIs on a test `PATH`: success, not installed,
//! signed out, expired, unusual characters, approval, and no token in errors,
//! profile JSON or exports. No cloud account is involved.
#![cfg(unix)]
use openrows_store::profile::{
    CloudAuth, CloudAuthError, CloudInvocation, CommandCancellation, CommandTrust,
    ConnectionProfile, Driver, EnvKind, EnvTag, PasswordStorage, ServerParams, TlsMode, exchange,
};
use std::{
    fs,
    os::unix::fs::PermissionsExt as _,
    path::{Path, PathBuf},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const HOST: &str = "db.abc.us-east-1.rds.amazonaws.com";
/// As awkward as real presigned URLs and JWTs, plus shell and format metacharacters.
const UNUSUAL: &str = r#"db.rds:5432/?Action=connect&X-Amz-Signature=a+b%2F=='"\$(touch x)`id`;é✓"#;

fn profile(host: &str, driver: Driver, auth: CloudAuth) -> ConnectionProfile {
    let params = ServerParams {
        host: host.to_owned(),
        port: 5432,
        user: Some("iam_user".to_owned()),
        ..ServerParams::default()
    };
    let mut profile = ConnectionProfile::new("cloud", driver, params, EnvTag::new(EnvKind::Dev));
    profile.password_storage = PasswordStorage::Cloud;
    profile.cloud_auth = Some(auth);
    profile.tls.mode = TlsMode::VerifyFull;
    profile
}

fn aws() -> CloudAuth {
    CloudAuth::Aws {
        region: "us-east-1".into(),
        profile: Some("dev".into()),
    }
}

/// A directory holding one fake CLI, the only entry of the test `PATH`.
struct FakeCli {
    dir: tempfile::TempDir,
    dirs: Vec<PathBuf>,
}

impl FakeCli {
    fn new(name: &str, body: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n"))?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
        let dirs = vec![dir.path().to_path_buf()];
        Ok(Self { dir, dirs })
    }

    fn output_file(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    /// Run the CLI for `profile`, approved, returning what it produced.
    fn token(
        &self,
        profile: &ConnectionProfile,
    ) -> Result<openrows_secrets::SecretString, CloudAuthError> {
        let invocation = CloudInvocation::new(profile, None, &self.dirs)?;
        let trust = CommandTrust::new(&self.dir.path().join("trust.json"));
        invocation.trust(&trust, profile.id)?;
        invocation.run(&trust, profile.id, &CommandCancellation::default())
    }
}

fn no_tool(result: &Result<CloudInvocation, CloudAuthError>) -> bool {
    matches!(result, Err(CloudAuthError::NotInstalled { tool: "aws" }))
}

#[test]
fn a_token_is_returned_byte_for_byte() -> TestResult {
    let body = format!("/bin/cat <<'TOKEN'\n{UNUSUAL}\nTOKEN");
    let cli = FakeCli::new("aws", &body)?;
    assert_eq!(
        cli.token(&profile(HOST, Driver::Postgres, aws()))?.expose(),
        UNUSUAL
    );
    Ok(())
}

#[test]
fn the_token_needs_no_trailing_newline_like_gcloud_prints_it() -> TestResult {
    let cli = FakeCli::new("gcloud", "printf 'ya29.synthetic-token'")?;
    let auth = CloudAuth::Google { account: None };
    assert_eq!(
        cli.token(&profile("10.0.0.5", Driver::Postgres, auth))?
            .expose(),
        "ya29.synthetic-token"
    );
    Ok(())
}

#[test]
fn the_cli_gets_exactly_the_documented_arguments_and_the_search_path() -> TestResult {
    let dir = tempfile::tempdir()?;
    let seen = dir.path().join("seen");
    let body = format!(
        "printf '%s\\n' \"$@\" > '{0}'\nprintf 'PATH=%s\\n' \"$PATH\" >> '{0}'\necho token",
        seen.display()
    );
    let cli = FakeCli::new("az", &body)?;
    let auth = CloudAuth::Azure {
        tenant: Some("contoso.example".into()),
    };
    cli.token(&profile("x.database.windows.net", Driver::Sqlserver, auth))?;
    let text = fs::read_to_string(&seen)?;
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        lines[..lines.len() - 1],
        [
            "account",
            "get-access-token",
            "--resource",
            "https://database.windows.net/",
            "--tenant",
            "contoso.example",
            "--query",
            "accessToken",
            "--output",
            "tsv",
        ]
    );
    assert_eq!(
        lines[lines.len() - 1],
        format!("PATH={}", cli.dirs[0].display())
    );
    Ok(())
}

#[test]
fn a_missing_tool_is_not_installed() -> TestResult {
    let empty = tempfile::tempdir()?;
    let dirs = [empty.path().to_path_buf()];
    let profile = profile(HOST, Driver::Postgres, aws());
    assert!(no_tool(&CloudInvocation::new(&profile, None, &dirs)));
    assert!(no_tool(&CloudInvocation::new(
        &profile,
        Some(Path::new("/no/such/aws")),
        &[]
    )));
    Ok(())
}

#[test]
fn signed_out_and_expired_sessions_name_the_sign_in_command() -> TestResult {
    let cases = [
        (
            "aws",
            aws(),
            "echo 'Unable to locate credentials.' >&2; exit 255",
            "aws sso login",
        ),
        (
            "aws",
            aws(),
            "echo 'An error occurred (ExpiredToken): the token is expired' >&2; exit 254",
            "aws sso login",
        ),
        (
            "az",
            CloudAuth::Azure { tenant: None },
            "echo \"ERROR: Please run 'az login' to setup account.\" >&2; exit 1",
            "az login",
        ),
        (
            "gcloud",
            CloudAuth::Google { account: None },
            "echo 'ERROR: There was a problem refreshing your current auth tokens' >&2; exit 1",
            "gcloud auth login",
        ),
    ];
    for (tool, auth, body, login) in cases {
        let cli = FakeCli::new(tool, body)?;
        let error = cli
            .token(&profile("10.0.0.5", Driver::Postgres, auth))
            .err();
        assert!(
            matches!(error, Some(CloudAuthError::SignedOut { .. })),
            "{error:?}"
        );
        let text = error.map(|error| error.to_string()).unwrap_or_default();
        assert!(text.contains(login), "{text}");
    }
    Ok(())
}

#[test]
fn wrong_settings_and_denied_identities_are_typed() -> TestResult {
    let wrong = FakeCli::new(
        "aws",
        "echo 'The config profile (dev) could not be found' >&2; exit 255",
    )?;
    let error = wrong.token(&profile(HOST, Driver::Postgres, aws())).err();
    assert!(
        matches!(error, Some(CloudAuthError::WrongTarget { .. })),
        "{error:?}"
    );
    let denied = FakeCli::new("gcloud", "echo 'ERROR: PERMISSION_DENIED' >&2; exit 1")?;
    let google = CloudAuth::Google { account: None };
    let error = denied
        .token(&profile("10.0.0.5", Driver::Mysql, google))
        .err();
    assert!(
        matches!(error, Some(CloudAuthError::PermissionDenied { .. })),
        "{error:?}"
    );
    Ok(())
}

#[test]
fn empty_or_silent_output_is_a_failure_not_an_empty_password() -> TestResult {
    let cli = FakeCli::new("aws", "exit 0")?;
    let error = cli.token(&profile(HOST, Driver::Postgres, aws())).err();
    assert!(
        matches!(error, Some(CloudAuthError::Failed { .. })),
        "{error:?}"
    );
    let cli = FakeCli::new("aws", "printf 'a\\tb\\n'")?;
    let error = cli.token(&profile(HOST, Driver::Postgres, aws())).err();
    assert!(
        matches!(error, Some(CloudAuthError::Failed { .. })),
        "{error:?}"
    );
    Ok(())
}

#[test]
fn no_token_runs_or_leaks_without_approval_of_this_destination() -> TestResult {
    let dir = tempfile::tempdir()?;
    let ran = dir.path().join("ran");
    let cli = FakeCli::new(
        "az",
        &format!("/usr/bin/touch '{}'\necho secret", ran.display()),
    )?;
    let trust = CommandTrust::new(&dir.path().join("trust.json"));
    let auth = CloudAuth::Azure { tenant: None };
    let mut profile = profile("mine.postgres.database.azure.com", Driver::Postgres, auth);
    let cancel = CommandCancellation::default();
    let invocation = CloudInvocation::new(&profile, None, &cli.dirs)?;
    assert!(matches!(
        invocation.run(&trust, profile.id, &cancel),
        Err(CloudAuthError::Untrusted)
    ));
    assert!(!ran.exists(), "ran before approval");
    invocation.trust(&trust, profile.id)?;
    assert_eq!(
        invocation.run(&trust, profile.id, &cancel)?.expose(),
        "secret"
    );
    profile.params.host = "attacker.example.com".into();
    let moved = CloudInvocation::new(&profile, None, &cli.dirs)?;
    assert!(
        !moved.is_trusted(&trust, profile.id)?,
        "approval follows the host"
    );
    assert!(matches!(
        moved.run(&trust, profile.id, &cancel),
        Err(CloudAuthError::Untrusted)
    ));
    Ok(())
}

#[test]
fn a_cancelled_request_never_starts_the_cli() -> TestResult {
    let cli = FakeCli::new("aws", "echo token")?;
    let profile = profile(HOST, Driver::Postgres, aws());
    let invocation = CloudInvocation::new(&profile, None, &cli.dirs)?;
    let trust = CommandTrust::new(&cli.output_file("trust.json"));
    invocation.trust(&trust, profile.id)?;
    let cancel = CommandCancellation::default();
    cancel.cancel();
    assert!(matches!(
        invocation.run(&trust, profile.id, &cancel),
        Err(CloudAuthError::Cancelled)
    ));
    Ok(())
}

#[test]
fn the_token_is_never_in_errors_profile_json_or_exports() -> TestResult {
    let token = "synthetic-token-value-1234";
    let body = format!(
        "echo 'debug: Authorization: Bearer {token}' >&2\n\
         echo 'debug: https://h/?X-Amz-Security-Token={token}' >&2\n\
         echo 'debug: eyJ{token}' >&2\necho {token}\nexit 1"
    );
    let failing = FakeCli::new("aws", &body)?;
    let saved = profile(HOST, Driver::Postgres, aws());
    let error = failing.token(&saved).err().ok_or("expected a failure")?;
    for rendered in [error.to_string(), format!("{error:?}")] {
        assert!(!rendered.contains(token), "{rendered}");
    }
    let working = FakeCli::new("aws", &format!("echo {token}"))?;
    let secret = working.token(&saved)?;
    assert_eq!(secret.expose(), token);
    assert!(!format!("{secret:?}").contains(token));
    let exported = exchange::export(std::slice::from_ref(&saved))?;
    let stored = serde_json::to_string(&saved)?;
    for text in [&exported, &stored] {
        assert!(!text.contains(token), "{text}");
        assert!(
            text.contains(r#""passwordStorage":"cloud""#)
                || text.contains(r#""passwordStorage": "cloud""#)
        );
    }
    Ok(())
}

#[test]
fn settings_round_trip_through_profile_json_and_exports() -> TestResult {
    let original = profile(HOST, Driver::Postgres, aws());
    let reloaded: ConnectionProfile = serde_json::from_str(&serde_json::to_string(&original)?)?;
    assert_eq!(reloaded, original);
    let exported = exchange::export(std::slice::from_ref(&original))?;
    assert!(exported.contains(r#""provider": "aws""#), "{exported}");
    assert!(exported.contains(r#""region": "us-east-1""#), "{exported}");
    let batch = exchange::parse(&exported)?;
    let imported = &batch.entries.first().ok_or("no entry")?.profile;
    assert_eq!(imported.cloud_auth, original.cloud_auth);
    assert_eq!(imported.password_storage, PasswordStorage::Cloud);
    assert_ne!(
        imported.id, original.id,
        "imports get a fresh id, hence no approval"
    );
    assert!(batch.warnings.is_empty(), "{:?}", batch.warnings);
    let unsupported = exported.replace(r#""driver": "postgres""#, r#""driver": "clickhouse""#);
    assert!(
        exchange::parse(&unsupported).is_err(),
        "engine without this provider"
    );
    Ok(())
}
