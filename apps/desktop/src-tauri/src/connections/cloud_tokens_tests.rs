// SPDX-License-Identifier: Apache-2.0
//! The real session path against a real server: a fake CLI's token reaches the
//! database over verified TLS, and a reconnect fetches a new one after the old
//! one stopped working. `just test-pg` runs it; no cloud account is involved.
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use openrows_core::{Resilience, RetryPolicy, SessionManager, SessionState};
use openrows_driver_api::DiscardSink;
use openrows_results::ResultStore;
use openrows_sql::Dialect;
use openrows_store::profile::{
    CloudAuth, CommandTrust, Driver, EnvKind, EnvTag, PasswordStorage, ServerParams, TlsMode,
};
use tokio::sync::oneshot;

use super::*;
use crate::connections::resilient;
use crate::connections::target::{self, DriverSecrets};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const ROLE: &str = "cloud_token_user";
/// Quotes, a backslash, shell and URL characters: the worst a real token could hold.
const TOKEN_1: &str = r#"tok1:db/?Action=connect&Sig=a+b%2F=='"\$(id)`x`;é"#;
const TOKEN_2: &str = r#"tok2:db/?Action=connect&Sig=c+d%2F=='"\$(id)`y`;é"#;

/// Tokens the fake CLI hands out, and how often it was asked.
struct Cli {
    dir: tempfile::TempDir,
}

impl Cli {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let script = format!(
            "#!/bin/sh\necho call >> '{0}/calls'\n/bin/cat '{0}/token'\n",
            dir.path().display()
        );
        let path = dir.path().join("aws");
        std::fs::write(&path, script)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
        Ok(Self { dir })
    }

    fn set_token(&self, token: &str) -> io::Result<()> {
        std::fs::write(self.dir.path().join("token"), format!("{token}\n"))
    }

    fn calls(&self) -> usize {
        std::fs::read_to_string(self.dir.path().join("calls"))
            .map_or(0, |text| text.lines().count())
    }
}

fn env(name: &str) -> Result<String, Box<dyn std::error::Error>> {
    std::env::var(name).map_err(|_| format!("{name} is not set; run `just test-pg`").into())
}

fn profile(port: u16, tls_dir: &Path) -> ConnectionProfile {
    let params = ServerParams {
        host: "localhost".to_owned(),
        port,
        database: Some("postgres".to_owned()),
        user: Some(ROLE.to_owned()),
        ..ServerParams::default()
    };
    let mut profile =
        ConnectionProfile::new("cloud", Driver::Postgres, params, EnvTag::new(EnvKind::Dev));
    profile.password_storage = PasswordStorage::Cloud;
    profile.cloud_auth = Some(CloudAuth::Aws {
        region: "us-east-1".to_owned(),
        profile: None,
    });
    profile.tls.mode = TlsMode::VerifyFull;
    profile.tls.ca_file = Some(tls_dir.join("ca.crt"));
    profile
}

fn quoted(token: &str) -> String {
    format!("'{}'", token.replace('\'', "''"))
}

/// Runs statements as the superuser, as a provider's token service would.
async fn admin(port: u16, statements: &[&str]) -> TestResult {
    let mut profile = profile(port, Path::new("/unused"));
    profile.password_storage = PasswordStorage::Keychain;
    profile.cloud_auth = None;
    profile.params.user = Some("postgres".to_owned());
    profile.tls.mode = TlsMode::Disable;
    profile.tls.ca_file = None;
    let secrets = DriverSecrets {
        password: Some(env("OPENROWS_TEST_PG_PASSWORD")?.as_str().into()),
        tls_key_passphrase: None,
    };
    let connected = target::connect(&target::target(&profile, &secrets)?).await?;
    let mut connection = connected.connection;
    for sql in statements {
        connection.execute(sql, &mut DiscardSink).await?;
    }
    connection.close().await?;
    Ok(())
}

/// What `fresh_password` does, without the Tauri state it reads settings from.
fn opened(invocation: CloudInvocation, trust: Arc<CommandTrust>, id: ProfileId) -> FreshPassword {
    Arc::new(move || {
        let (invocation, trust) = (invocation.clone(), Arc::clone(&trust));
        Box::pin(async move {
            let token = tokio::task::spawn_blocking(move || {
                invocation.run(&trust, id, &CommandCancellation::default())
            })
            .await
            .map_err(|error| DriverError::Auth(error.to_string()))?
            .map_err(|error| reconnect_failure(&error))?;
            Ok(token.expose().into())
        })
    })
}

async fn reaches(
    changes: &mut tokio::sync::broadcast::Receiver<openrows_core::SessionEvent>,
    state: SessionState,
) -> TestResult {
    let wait = async {
        loop {
            let change = changes.recv().await?;
            if change.state == state {
                return Ok::<_, Box<dyn std::error::Error>>(());
            }
        }
    };
    tokio::time::timeout(Duration::from_secs(30), wait).await?
}

async fn runs(manager: &SessionManager, session: openrows_types::SessionId) -> TestResult {
    let mut handle = manager.execute(session, "SELECT 1")?;
    loop {
        let event = tokio::time::timeout(Duration::from_secs(30), handle.next_event())
            .await?
            .ok_or("no terminal event")?;
        if event.is_terminal() {
            if let openrows_core::QueryEvent::Failed { error } = &event {
                return Err(error.message.clone().into());
            }
            return Ok(());
        }
    }
}

/// The driver-level check that the old token no longer logs in.
async fn logs_in(
    profile: &ConnectionProfile,
    token: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    let secrets = DriverSecrets {
        password: Some(token.into()),
        tls_key_passphrase: None,
    };
    Ok(target::connect(&target::target(profile, &secrets)?)
        .await
        .is_ok())
}

#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Captured {
    fn text(&self) -> String {
        let bytes = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

impl io::Write for Captured {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Open a resilient session on `first` the way `open_session` does.
async fn open_session(
    manager: &SessionManager,
    profile: &ConnectionProfile,
    first: SecretString,
    fresh: FreshPassword,
) -> Result<openrows_types::SessionId, Box<dyn std::error::Error>> {
    let secrets = DriverSecrets {
        password: Some(first),
        tls_key_passphrase: None,
    };
    let session_target = target::target(profile, &secrets)?;
    let (server, received) = oneshot::channel();
    let recipe = resilient::recipe(session_target, Dialect::Postgres, server, Some(fresh));
    let session = manager
        .open_resilient(Resilience {
            policy: RetryPolicy {
                probe_timeout: Duration::from_millis(1000),
                first_delay: Duration::from_millis(100),
                ..RetryPolicy::default()
            },
            ..recipe
        })
        .await?;
    assert_eq!(received.await?.database, "postgres");
    Ok(session)
}

/// The provider moves on: the old token stops working, the CLI issues a new
/// one and every existing connection of the role is cut.
async fn expire_token(cli: &Cli, port: u16, profile: &ConnectionProfile) -> TestResult {
    cli.set_token(TOKEN_2)?;
    let rotate = format!("ALTER ROLE {ROLE} PASSWORD {}", quoted(TOKEN_2));
    let terminate =
        format!("SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE usename = '{ROLE}'");
    admin(port, &[&rotate, &terminate]).await?;
    assert!(
        !logs_in(profile, TOKEN_1).await?,
        "the old token must be expired"
    );
    assert!(logs_in(profile, TOKEN_2).await?);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "needs PostgreSQL with TLS; run with `just test-pg`"]
async fn cloud_token_reaches_the_server_and_reconnect_fetches_a_new_one() -> TestResult {
    let port: u16 = env("OPENROWS_TEST_PG_PORT")?.parse()?;
    let tls_dir = std::path::PathBuf::from(env("OPENROWS_TEST_PG_TLS_DIR")?);
    let captured = Captured::default();
    let writer = captured.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(move || writer.clone())
        .finish();
    let _logs = tracing::subscriber::set_default(subscriber);

    let cli = Cli::new()?;
    cli.set_token(TOKEN_1)?;
    let create = format!("CREATE ROLE {ROLE} LOGIN PASSWORD {}", quoted(TOKEN_1));
    let drop_old = format!("DROP ROLE IF EXISTS {ROLE}");
    admin(port, &[&drop_old, &create]).await?;

    let profile = profile(port, &tls_dir);
    let trust = Arc::new(CommandTrust::new(&cli.dir.path().join("trust.json")));
    let dirs = [cli.dir.path().to_path_buf()];
    let invocation = CloudInvocation::new(&profile, None, &dirs)?;
    invocation.trust(&trust, profile.id)?;
    let first = invocation.run(&trust, profile.id, &CommandCancellation::default())?;
    assert_eq!(first.expose(), TOKEN_1);
    assert_eq!(cli.calls(), 1);

    let manager = SessionManager::new(Arc::new(ResultStore::default()));
    manager.set_keepalive(Some(Duration::from_millis(500)));
    let mut changes = manager.subscribe();
    let fresh = opened(invocation, Arc::clone(&trust), profile.id);
    let session = open_session(&manager, &profile, first, fresh).await?;
    assert_eq!(
        cli.calls(),
        1,
        "the opening connect uses the token it was given"
    );
    runs(&manager, session).await?;

    expire_token(&cli, port, &profile).await?;
    reaches(&mut changes, SessionState::Reconnecting).await?;
    reaches(&mut changes, SessionState::Idle).await?;
    assert_eq!(cli.calls(), 2, "the reconnect asked the CLI again");
    runs(&manager, session).await?;
    manager.close(session).await?;
    admin(port, &[&drop_old]).await?;

    let logs = captured.text();
    assert!(
        !logs.is_empty(),
        "the capture must have seen the connect logs"
    );
    assert!(
        !logs.contains("tok1") && !logs.contains("tok2"),
        "token in logs"
    );
    Ok(())
}

#[test]
fn every_failure_keeps_its_advice_at_the_ipc_boundary() {
    let insecure = IpcError::from(CloudAuthError::InsecureRoute {
        reason: "it could travel unencrypted",
        fix: "Choose Verify full.",
    });
    assert_eq!(insecure.kind, IpcErrorKind::Tls);
    assert!(insecure.message.contains("Verify full"));
    let missing = IpcError::from(CloudAuthError::NotInstalled { tool: "gcloud" });
    assert_eq!(missing.kind, IpcErrorKind::NotFound);
    assert!(missing.message.contains("Settings"));
    let signed_out = IpcError::from(CloudAuthError::SignedOut {
        tool: "az",
        login: "az login",
    });
    assert_eq!(signed_out.kind, IpcErrorKind::Auth);
    assert!(signed_out.message.contains("az login"));
}

#[test]
fn reconnects_retry_only_what_can_pass_by_itself() {
    assert!(matches!(
        reconnect_failure(&CloudAuthError::Timeout { tool: "az" }),
        DriverError::Network(_)
    ));
    assert!(matches!(
        reconnect_failure(&CloudAuthError::Failed {
            tool: "az",
            detail: "connection reset".into()
        }),
        DriverError::Network(_)
    ));
    assert!(matches!(
        reconnect_failure(&CloudAuthError::SignedOut {
            tool: "az",
            login: "az login"
        }),
        DriverError::Auth(_)
    ));
    assert!(matches!(
        reconnect_failure(&CloudAuthError::Untrusted),
        DriverError::Auth(_)
    ));
}

#[test]
fn the_approval_question_names_the_destination_and_exact_command() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("az");
    std::fs::write(&path, "#!/bin/sh\n")?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
    let mut profile = profile(5432, dir.path());
    profile.params.host = "mine.postgres.database.azure.com".to_owned();
    profile.cloud_auth = Some(CloudAuth::Azure { tenant: None });
    let dirs = [dir.path().to_path_buf()];
    let text = approval_text(&CloudInvocation::new(&profile, None, &dirs)?)?;
    assert!(
        text.contains("mine.postgres.database.azure.com:5432"),
        "{text}"
    );
    assert!(text.contains("get-access-token"), "{text}");
    Ok(())
}
