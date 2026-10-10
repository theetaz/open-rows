// SPDX-License-Identifier: Apache-2.0
//! Native showplan through the actor, with rollback and session-state guarantees.
#[path = "sqlserver/support.rs"]
mod sqlserver_explain_support;
use openrows_core::SessionManager;
use openrows_core::explain::{
    Admission, ExplainRequest, Explained, admit, confirmation_token, explain,
};
use openrows_core::safety::SafetyPolicy;
use openrows_core::script::ScriptCanceller;
use openrows_explain::{Engine, Mode};
use openrows_results::ResultStore;
use openrows_sql::Dialect;
use openrows_types::SessionId;
use sqlserver_explain_support::{TestResult, open, run};
use std::sync::Arc;

const READ: &str = "SELECT * FROM explain_t";
const UPDATE: &str = "UPDATE explain_t SET v=0 WHERE id<=10";

async fn explain_on(
    manager: &SessionManager,
    session: SessionId,
    mode: Mode,
    sql: &str,
) -> TestResult<Explained> {
    Ok(explain(
        manager,
        session,
        ExplainRequest {
            engine: Engine::SqlServer,
            mode,
            statement: sql,
        },
        &ScriptCanceller::default(),
    )
    .await?)
}

fn policy(read_only: bool) -> SafetyPolicy {
    SafetyPolicy {
        mode: if read_only {
            openrows_types::safety::Safety::ReadOnly
        } else {
            openrows_types::safety::Safety::Unrestricted
        },
        dialect: Dialect::SqlServer,
    }
}

async fn unchanged(manager: &SessionManager, session: SessionId) -> TestResult {
    run(
        manager,
        session,
        "IF (SELECT SUM(v) FROM explain_t) <> 200010000 THROW 50000, 'Explain changed rows', 1",
    )
    .await
}

fn capture(name: &str, plan: &Explained) -> TestResult {
    if std::env::var_os("OPENROWS_CAPTURE_SHOWPLAN").is_some() {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../openrows-explain/fixtures");
        std::fs::write(dir.join(format!("ms_{name}.xml")), &plan.raw)?;
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs SQL Server; run just test-mssql"]
async fn sqlserver_explain_analyze_rollback_confirmation_and_session_restoration() -> TestResult {
    let manager = SessionManager::new(Arc::new(ResultStore::default()));
    let session = Box::pin(open(&manager)).await?;
    read_plans(&manager, session).await?;
    writes(&manager, session).await?;
    error_restores(&manager, session).await?;
    existing_transaction(&manager, session).await?;
    run(
        &manager,
        session,
        "DROP TABLE explain_t; DROP TABLE explain_keys",
    )
    .await?;
    manager.close(session).await?;
    Ok(())
}

async fn read_plans(manager: &SessionManager, session: SessionId) -> TestResult {
    for sql in [
        "CREATE TABLE explain_t (id int NOT NULL, v int NOT NULL)",
        "INSERT INTO explain_t SELECT TOP (20000) ROW_NUMBER() OVER(ORDER BY (SELECT NULL)), ROW_NUMBER() OVER(ORDER BY (SELECT NULL)) FROM sys.all_objects a CROSS JOIN sys.all_objects b",
        "CREATE TABLE explain_keys (id int NOT NULL PRIMARY KEY)",
        "INSERT INTO explain_keys SELECT id FROM explain_t",
        "UPDATE STATISTICS explain_t WITH FULLSCAN",
    ] {
        run(manager, session, sql).await?;
    }
    for (name, sql) in [
        ("scan", READ),
        ("seek", "SELECT * FROM explain_keys WHERE id=10"),
        (
            "join",
            "SELECT t.* FROM explain_t t INNER HASH JOIN explain_keys k ON k.id=t.id OPTION (MAXDOP 1, MAX_GRANT_PERCENT=0.001)",
        ),
    ] {
        let plan = explain_on(manager, session, Mode::Plan, sql).await?;
        assert!(!plan.plan.analyzed && !plan.rolled_back);
        if name != "join" {
            capture(name, &plan)?;
        }
        let actual = explain_on(manager, session, Mode::Analyze, sql).await?;
        assert!(actual.plan.analyzed && actual.rolled_back);
        if name == "join" {
            capture(name, &actual)?;
        }
        assert!(
            actual
                .plan
                .nodes
                .iter()
                .any(|node| node.data.actual_rows.is_some())
        );
    }
    Ok(())
}

async fn writes(manager: &SessionManager, session: SessionId) -> TestResult {
    assert_eq!(
        admit(policy(false), Engine::SqlServer, Mode::Analyze, UPDATE),
        Admission::Confirm
    );
    assert!(matches!(
        admit(policy(true), Engine::SqlServer, Mode::Analyze, UPDATE),
        Admission::Block(_)
    ));
    let token = confirmation_token(policy(false), &session.to_string(), UPDATE);
    assert_ne!(
        token,
        confirmation_token(policy(false), &session.to_string(), READ)
    );
    let estimated = explain_on(manager, session, Mode::Plan, UPDATE).await?;
    assert!(!estimated.rolled_back);
    unchanged(manager, session).await?;
    let update = explain_on(manager, session, Mode::Analyze, UPDATE).await?;
    assert!(update.rolled_back);
    capture("update", &update)?;
    unchanged(manager, session).await?;
    Ok(())
}

async fn error_restores(manager: &SessionManager, session: SessionId) -> TestResult {
    for mode in [Mode::Plan, Mode::Analyze] {
        assert!(
            explain_on(
                manager,
                session,
                mode,
                "SELECT * FROM no_such_explain_table"
            )
            .await
            .is_err()
        );
        // This would return XML rather than a scalar if SHOWPLAN were left on.
        run(
            manager,
            session,
            "IF @@TRANCOUNT<>0 THROW 50000, 'Leaked transaction', 1",
        )
        .await?;
        let mut result = manager.execute(session, "SELECT 123 AS restored")?;
        let mut normal = false;
        while let Some(event) = result.next_event().await {
            if let openrows_core::QueryEvent::ResultBegin { schema, .. } = event {
                normal = schema.field(0).name() == "restored";
            }
        }
        assert!(normal, "showplan mode remained enabled");
    }
    Ok(())
}

async fn existing_transaction(manager: &SessionManager, session: SessionId) -> TestResult {
    run(manager, session, "BEGIN TRAN").await?;
    run(
        manager,
        session,
        "INSERT INTO explain_t VALUES (20001, 20001)",
    )
    .await?;
    explain_on(manager, session, Mode::Analyze, UPDATE).await?;
    run(manager, session, "IF @@TRANCOUNT<>1 OR (SELECT SUM(v) FROM explain_t)<>200030001 THROW 50000, 'Savepoint lost user work', 1").await?;
    run(manager, session, "ROLLBACK TRAN").await?;
    unchanged(manager, session).await
}
