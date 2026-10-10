// SPDX-License-Identifier: Apache-2.0
//! M1-01 statement timeouts against a real server: the session cancels a
//! statement past its client-side limit and reports a timeout (not a user
//! cancel), and the server-side `statement_timeout` set at session start is
//! reported the same way and set again after a reconnect.
use super::resilience::{KEEPALIVE, PROBE_TIMEOUT, backend_pid, reaches, reconnect, terminate};
use super::{TestResult, config, events, open, single_label};
use openrows_core::{ErrorKind, QueryEvent, Resilience, RetryPolicy, SessionManager, SessionState};
use openrows_results::ResultStore;
use openrows_sql::Dialect;
use std::sync::Arc;
use std::time::{Duration, Instant};

const LIMIT: Duration = Duration::from_millis(500);

/// The terminal event of `sql` on `session`.
async fn last(
    manager: &SessionManager,
    session: openrows_types::SessionId,
    sql: &str,
) -> TestResult<QueryEvent> {
    let mut handle = manager.execute(session, sql)?;
    Ok(events(&mut handle).await?.pop().ok_or("no events")?)
}

fn timeout_error(event: &QueryEvent) -> Option<&openrows_core::CoreError> {
    match event {
        QueryEvent::Failed { error } if error.kind == ErrorKind::Timeout => Some(error),
        _ => None,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs PostgreSQL; run with `just test-pg`"]
async fn a_statement_past_the_client_timeout_is_cancelled_as_a_timeout() -> TestResult {
    let manager = SessionManager::new(Arc::new(ResultStore::default()));
    let session = open(&manager).await?;
    manager.set_statement_timeout(session, Some(LIMIT))?;
    let started = Instant::now();
    let event = last(&manager, session, "SELECT pg_sleep(30)").await?;
    let error = timeout_error(&event).ok_or_else(|| format!("{event:?}"))?;
    assert!(error.message.contains("statement timeout"), "{error}");
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
    assert!(matches!(
        last(&manager, session, "SELECT 1").await?,
        QueryEvent::Completed { .. }
    ));
    manager.close(session).await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs PostgreSQL; run with `just test-pg`"]
async fn the_server_statement_timeout_is_set_at_start_and_after_a_reconnect() -> TestResult {
    let store = Arc::new(ResultStore::default());
    let manager = SessionManager::new(store.clone());
    manager.set_keepalive(Some(KEEPALIVE));
    let mut changes = manager.subscribe();
    let recipe = Resilience {
        reconnect: reconnect(config()?),
        dialect: Some(Dialect::Postgres),
        policy: RetryPolicy {
            probe_timeout: PROBE_TIMEOUT,
            first_delay: Duration::from_millis(100),
            ..RetryPolicy::default()
        },
        setup: vec!["SET statement_timeout = 500".to_owned()],
    };
    let session = manager.open_resilient(recipe).await?;
    let event = last(&manager, session, "SELECT pg_sleep(30)").await?;
    let error = timeout_error(&event).ok_or_else(|| format!("{event:?}"))?;
    assert_eq!(error.db_code.as_deref(), Some("57014"));
    let pid = backend_pid(&manager, &store, session).await?;
    terminate(&manager, &pid).await?;
    reaches(&mut changes, session, SessionState::Reconnecting).await?;
    reaches(&mut changes, session, SessionState::Idle).await?;
    let sql = "SELECT 1 AS n, current_setting('statement_timeout') AS label";
    assert_eq!(
        single_label(&manager, &store, session, sql).await?,
        ["500ms"]
    );
    manager.close(session).await?;
    Ok(())
}

/// Connects through the driver contract, so the request's startup commands
/// run inside every connect, the first and each reconnect.
fn reconnect_with_startup(startup_commands: Vec<String>) -> TestResult<openrows_core::Reconnect> {
    use openrows_driver_api::{
        ConnectRequest, ConnectTimeouts, ConnectTrace, Driver, Endpoint, Session, SslMode, TlsFiles,
    };
    let config = config()?;
    let request = Arc::new(ConnectRequest {
        endpoint: Endpoint::Server {
            host: config.host,
            port: config.port,
        },
        database: config.database,
        user: config.user,
        password: config
            .password
            .as_deref()
            .map(openrows_net::SecretString::from),
        application_name: "openrows-test".to_owned(),
        ssl_mode: SslMode::Prefer,
        tls: TlsFiles::default(),
        via: None,
        timeouts: ConnectTimeouts::default(),
        startup_commands,
        driver_options: std::collections::BTreeMap::new(),
    });
    Ok(Arc::new(move || {
        let request = Arc::clone(&request);
        Box::pin(async move {
            let connection = openrows_driver_postgres::PgDriver
                .connect(&request, &mut ConnectTrace::default())
                .await?;
            let session: Box<dyn Session> = connection;
            Ok(session)
        })
    }))
}

/// Startup commands run first, then the timeout setup (which wins for
/// `statement_timeout`), on the first connect and again after a reconnect.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs PostgreSQL; run with `just test-pg`"]
async fn startup_commands_run_before_the_timeout_setup_on_every_connect() -> TestResult {
    let store = Arc::new(ResultStore::default());
    let manager = SessionManager::new(store.clone());
    manager.set_keepalive(Some(KEEPALIVE));
    let mut changes = manager.subscribe();
    let startup = vec![
        "SET statement_timeout = 9000".to_owned(),
        "SET application_name = 'orc_boot'".to_owned(),
    ];
    let recipe = Resilience {
        reconnect: reconnect_with_startup(startup)?,
        dialect: Some(Dialect::Postgres),
        policy: RetryPolicy {
            probe_timeout: PROBE_TIMEOUT,
            first_delay: Duration::from_millis(100),
            ..RetryPolicy::default()
        },
        setup: vec!["SET statement_timeout = 500".to_owned()],
    };
    let session = manager.open_resilient(recipe).await?;
    let sql = "SELECT 1 AS n, current_setting('statement_timeout') || '/' || \
               current_setting('application_name') AS label";
    assert_eq!(
        single_label(&manager, &store, session, sql).await?,
        ["500ms/orc_boot"]
    );
    let pid = backend_pid(&manager, &store, session).await?;
    terminate(&manager, &pid).await?;
    reaches(&mut changes, session, SessionState::Reconnecting).await?;
    reaches(&mut changes, session, SessionState::Idle).await?;
    assert_eq!(
        single_label(&manager, &store, session, sql).await?,
        ["500ms/orc_boot"]
    );
    manager.close(session).await?;
    Ok(())
}
