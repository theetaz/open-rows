// SPDX-License-Identifier: Apache-2.0
//! M2-12 against a real server: a script file with a function body
//! (dollar quotes), DDL and data runs in one transaction; a failing file
//! rolls back its DDL, which `PostgreSQL` supports transactionally.
use super::{CountRows, TestResult, events, open};
use openrows_core::safety::SafetyPolicy;
use openrows_core::script::ScriptCanceller;
use openrows_core::script_file::{FileOutcome, FileRun, FileSummary, run_file, scan_file};
use openrows_core::{QueryEvent, SessionManager};
use openrows_sql::Dialect;
use openrows_types::SessionId;
use std::path::Path;
use std::sync::Arc;

const POLICY: SafetyPolicy = SafetyPolicy {
    read_only: false,
    confirm_writes: false,
    dialect: Dialect::Postgres,
};

const SCRIPT: &str = "DROP SCHEMA IF EXISTS m212 CASCADE;\n\
    CREATE SCHEMA m212;\n\
    CREATE TABLE m212.t (id int PRIMARY KEY, note text);\n\
    CREATE FUNCTION m212.twice(x int) RETURNS int LANGUAGE plpgsql AS $$\n\
    BEGIN\n  RETURN x * 2; -- inner ; stays inside\nEND;\n$$;\n\
    INSERT INTO m212.t SELECT g, 'n;' || m212.twice(g) FROM generate_series(1, 500) g;\n";

async fn run(manager: &SessionManager, session: SessionId, path: &Path) -> FileSummary {
    let run = FileRun {
        path,
        policy: POLICY,
        stop_on_error: true,
        transaction: true,
        stamp: None,
    };
    run_file(manager, session, run, &ScriptCanceller::default(), drop).await
}

async fn exists(manager: &SessionManager, session: SessionId) -> TestResult<bool> {
    let mut handle = manager.execute(session, "SELECT 1 FROM m212.t LIMIT 1")?;
    Ok(!events(&mut handle)
        .await?
        .iter()
        .any(|event| matches!(event, QueryEvent::Failed { .. })))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs PostgreSQL; run with `just test-pg`"]
async fn a_script_file_commits_or_rolls_back_as_one_transaction() -> TestResult {
    let manager = SessionManager::new(Arc::new(CountRows::default()));
    let session = open(&manager).await?;
    let directory = tempfile::tempdir()?;
    let good = directory.path().join("good.sql");
    std::fs::write(&good, SCRIPT)?;
    assert_eq!(scan_file(&good, POLICY, || false)?.statements, 5);
    let summary = run(&manager, session, &good).await;
    assert_eq!(summary.outcome, FileOutcome::Completed, "{summary:?}");
    assert_eq!((summary.statements, summary.rows_affected), (5, 500));
    assert!(exists(&manager, session).await?);

    let bad = directory.path().join("bad.sql");
    std::fs::write(
        &bad,
        format!("{SCRIPT}INSERT INTO m212.t VALUES (1, 'duplicate');\n"),
    )?;
    let summary = run(&manager, session, &bad).await;
    assert_eq!(summary.outcome, FileOutcome::Failed);
    assert!(summary.rolled_back);
    assert_eq!(summary.errors, 1);
    // The DROP SCHEMA at the top was rolled back with everything else.
    assert!(exists(&manager, session).await?);
    let mut handle = manager.execute(session, "DROP SCHEMA m212 CASCADE")?;
    events(&mut handle).await?;
    Ok(())
}
