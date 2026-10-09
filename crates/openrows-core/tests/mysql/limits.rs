// SPDX-License-Identifier: Apache-2.0
//! M1-01 statement timeouts against `MySQL` and `MariaDB`: the client-side
//! limit cancels through `KILL QUERY` and reports a timeout; the server-side
//! limit (`max_execution_time` on `MySQL`, `max_statement_time` on `MariaDB`)
//! set at session start is reported the same way and set again after a
//! reconnect.
use super::resilience::{
    KEEPALIVE, PROBE_TIMEOUT, Texts, connection_id, outcome, reaches, reconnect, text,
};
use super::{TestResult, open, run};
use openrows_core::{ErrorKind, QueryEvent, Resilience, RetryPolicy, SessionManager, SessionState};
use openrows_sql::Dialect;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Counts 10^9 rows, far longer than the limits; an interrupted `SLEEP` or
/// `BENCHMARK` returns a value instead of failing, a scan fails with
/// "interrupted".
const SLOW: &str = "SELECT count(*) FROM \
    (SELECT 0 d UNION ALL SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3 UNION ALL SELECT 4 \
     UNION ALL SELECT 5 UNION ALL SELECT 6 UNION ALL SELECT 7 UNION ALL SELECT 8 \
     UNION ALL SELECT 9) a, \
    (SELECT 0 d UNION ALL SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3 UNION ALL SELECT 4 \
     UNION ALL SELECT 5 UNION ALL SELECT 6 UNION ALL SELECT 7 UNION ALL SELECT 8 \
     UNION ALL SELECT 9) b, \
    (SELECT 0 d UNION ALL SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3 UNION ALL SELECT 4 \
     UNION ALL SELECT 5 UNION ALL SELECT 6 UNION ALL SELECT 7 UNION ALL SELECT 8 \
     UNION ALL SELECT 9) c, information_schema.columns x, information_schema.columns y";

fn mariadb() -> bool {
    std::env::var("OPENROWS_TEST_MYSQL_ENGINE").is_ok_and(|engine| engine == "mariadb")
}

fn is_timeout(event: &QueryEvent) -> bool {
    matches!(event, QueryEvent::Failed { error } if error.kind == ErrorKind::Timeout)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs MySQL or MariaDB; run with `just test-mysql`"]
async fn a_statement_past_the_client_timeout_is_cancelled_as_a_timeout() -> TestResult {
    let manager = SessionManager::new(Arc::new(Texts::default()));
    let session = open(&manager).await?;
    manager.set_statement_timeout(session, Some(Duration::from_millis(500)))?;
    let started = Instant::now();
    let event = outcome(&manager, session, SLOW).await?;
    assert!(is_timeout(&event), "{event:?}");
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
    run(&manager, session, "SELECT 1").await?;
    manager.close(session).await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs MySQL or MariaDB; run with `just test-mysql`"]
async fn the_server_statement_timeout_is_set_at_start_and_after_a_reconnect() -> TestResult {
    let texts = Arc::new(Texts::default());
    let manager = SessionManager::new(texts.clone());
    manager.set_keepalive(Some(KEEPALIVE));
    let mut changes = manager.subscribe();
    let (setup, variable, value) = if mariadb() {
        (
            "SET SESSION max_statement_time = 0.5",
            "@@max_statement_time",
            "0.500000",
        )
    } else {
        (
            "SET SESSION max_execution_time = 500",
            "@@max_execution_time",
            "500",
        )
    };
    let recipe = Resilience {
        reconnect: reconnect()?,
        dialect: Some(Dialect::MySql),
        policy: RetryPolicy {
            probe_timeout: PROBE_TIMEOUT,
            first_delay: Duration::from_millis(100),
            ..RetryPolicy::default()
        },
        setup: vec![setup.to_owned()],
    };
    let session = manager.open_resilient(recipe).await?;
    let event = outcome(&manager, session, SLOW).await?;
    assert!(is_timeout(&event), "{event:?}");
    let admin = open(&manager).await?;
    let id = connection_id(&manager, &texts, session).await?;
    run(&manager, admin, &format!("KILL {id}")).await?;
    reaches(&mut changes, session, SessionState::Reconnecting).await?;
    reaches(&mut changes, session, SessionState::Idle).await?;
    let sql = format!("SELECT CAST({variable} AS CHAR)");
    assert_eq!(text(&manager, &texts, session, &sql).await?, value);
    manager.close(session).await?;
    manager.close(admin).await?;
    Ok(())
}
