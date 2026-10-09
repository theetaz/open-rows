// SPDX-License-Identifier: Apache-2.0
//! Explain and Explain Analyze on a real server: plans parse into the model
//! and analysed writes are rolled back, also inside the user's transaction.
use super::{CountRows, TestResult, open};
use openrows_core::explain::{ExplainRequest, Explained, explain};
use openrows_core::script::ScriptCanceller;
use openrows_core::{QueryEvent, SessionManager};
use openrows_explain::{Engine, Hotspot, Mode, Weight};
use openrows_types::SessionId;
use std::sync::Arc;

async fn run(manager: &SessionManager, session: SessionId, sql: &str) -> TestResult {
    let mut handle = manager.execute(session, sql)?;
    while let Some(event) = handle.next_event().await {
        if let QueryEvent::Failed { error } = event {
            return Err(format!("{sql}: {}", error.message).into());
        }
    }
    Ok(())
}

/// Fails unless `explain_t` holds exactly `rows` rows.
async fn expect_rows(manager: &SessionManager, session: SessionId, rows: u32) -> TestResult {
    let check = format!(
        "DO $$ BEGIN IF (SELECT count(*) FROM explain_t) <> {rows} THEN \
         RAISE EXCEPTION 'row count changed'; END IF; END $$"
    );
    run(manager, session, &check).await
}

async fn explain_on(
    manager: &SessionManager,
    session: SessionId,
    mode: Mode,
    statement: &str,
) -> TestResult<Explained> {
    let request = ExplainRequest {
        engine: Engine::Postgres,
        mode,
        statement,
    };
    Ok(explain(manager, session, request, &ScriptCanceller::default()).await?)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs PostgreSQL; run with `just test-pg`"]
async fn explain_and_analyze_on_postgres_roll_writes_back() -> TestResult {
    let manager = SessionManager::new(Arc::new(CountRows::default()));
    let session = open(&manager).await?;
    for sql in [
        "DROP TABLE IF EXISTS explain_t",
        "CREATE TABLE explain_t (id int PRIMARY KEY, v int)",
        "INSERT INTO explain_t SELECT g, g FROM generate_series(1, 20000) g",
        "ANALYZE explain_t",
    ] {
        run(&manager, session, sql).await?;
    }
    let plan = explain_on(
        &manager,
        session,
        Mode::Plan,
        "SELECT * FROM explain_t WHERE v > 5",
    )
    .await?;
    assert!(!plan.plan.analyzed && !plan.rolled_back);
    assert_eq!(plan.plan.weight, Weight::Cost);
    assert_eq!(plan.plan.nodes[0].data.node_type, "Seq Scan");
    assert!(plan.raw.contains("\"Node Type\""));

    let read = explain_on(
        &manager,
        session,
        Mode::Analyze,
        "SELECT * FROM explain_t WHERE v > 5",
    )
    .await?;
    assert!(read.plan.analyzed && read.rolled_back);
    assert_eq!(read.plan.weight, Weight::Time);
    let scan = &read.plan.nodes[0];
    assert!(
        scan.hotspots.contains(&Hotspot::FullScan {
            rows: Some(20_000.0)
        }),
        "{:?}",
        scan.hotspots
    );

    let delete = explain_on(
        &manager,
        session,
        Mode::Analyze,
        "DELETE FROM explain_t WHERE id <= 10",
    )
    .await?;
    assert_eq!(delete.plan.nodes[0].data.node_type, "Delete");
    expect_rows(&manager, session, 20_000).await?;

    run(&manager, session, "BEGIN").await?;
    run(&manager, session, "INSERT INTO explain_t VALUES (20001, 1)").await?;
    explain_on(&manager, session, Mode::Analyze, "DELETE FROM explain_t").await?;
    expect_rows(&manager, session, 20_001).await?;
    run(&manager, session, "ROLLBACK").await?;
    expect_rows(&manager, session, 20_000).await?;
    run(&manager, session, "DROP TABLE explain_t").await
}
