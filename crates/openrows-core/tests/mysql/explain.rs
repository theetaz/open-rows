// SPDX-License-Identifier: Apache-2.0
//! Explain on `MySQL` (JSON plans, `EXPLAIN ANALYZE` trees) and `MariaDB`
//! (`EXPLAIN`/`ANALYZE FORMAT=JSON`) parses into the plan model.
use super::{TestResult, open, run};
use openrows_core::SessionManager;
use openrows_core::explain::{ExplainRequest, Explained, explain};
use openrows_core::script::ScriptCanceller;
use openrows_explain::{Engine, Format, Mode, Weight};
use openrows_results::ResultStore;
use openrows_types::SessionId;
use std::sync::Arc;

const QUERY: &str = "SELECT c.region, count(*) FROM explain_o o JOIN explain_c c ON c.id = o.c \
    WHERE c.region = 'eu' GROUP BY c.region";

fn engine() -> Engine {
    match std::env::var("OPENROWS_TEST_MYSQL_ENGINE").as_deref() {
        Ok("mariadb") => Engine::MariaDb,
        _ => Engine::MySql,
    }
}

async fn explain_on(
    manager: &SessionManager,
    session: SessionId,
    mode: Mode,
    statement: &str,
) -> TestResult<Explained> {
    let request = ExplainRequest {
        engine: engine(),
        mode,
        statement,
    };
    Ok(explain(manager, session, request, &ScriptCanceller::default()).await?)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs MySQL or MariaDB; run with `just test-mysql`"]
async fn explain_and_analyze_parse_on_mysql_and_mariadb() -> TestResult {
    let manager = SessionManager::new(Arc::new(ResultStore::default()));
    let session = open(&manager).await?;
    for sql in [
        "DROP TABLE IF EXISTS explain_o, explain_c",
        "CREATE TABLE explain_c (id int PRIMARY KEY, region varchar(8))",
        "CREATE TABLE explain_o (id int PRIMARY KEY, c int, KEY explain_o_c (c))",
        "INSERT INTO explain_c VALUES (1, 'eu'), (2, 'us'), (3, 'eu')",
        "INSERT INTO explain_o VALUES (1, 1), (2, 1), (3, 2), (4, 3)",
    ] {
        run(&manager, session, sql).await?;
    }
    let plan = explain_on(&manager, session, Mode::Plan, QUERY).await?;
    assert_eq!(plan.format, Format::MySqlJson);
    assert!(!plan.plan.analyzed && !plan.rolled_back);
    assert_eq!(plan.plan.weight, Weight::Cost);
    let tables: Vec<&str> = plan
        .plan
        .nodes
        .iter()
        .filter_map(|node| node.data.relation.as_deref())
        .collect();
    assert!(tables.contains(&"c") && tables.contains(&"o"), "{tables:?}");

    let update = explain_on(
        &manager,
        session,
        Mode::Plan,
        "UPDATE explain_o SET c = 2 WHERE id = 1",
    )
    .await?;
    assert!(
        update
            .plan
            .nodes
            .iter()
            .any(|node| node.data.relation.as_deref() == Some("explain_o"))
    );

    let analyzed = explain_on(&manager, session, Mode::Analyze, QUERY).await?;
    assert!(analyzed.plan.analyzed, "{}", analyzed.raw);
    assert_eq!(analyzed.plan.weight, Weight::Time);
    let scanned = analyzed
        .plan
        .nodes
        .iter()
        .any(|node| node.data.actual_rows.is_some());
    assert!(scanned, "{}", analyzed.raw);
    run(&manager, session, "DROP TABLE explain_o, explain_c").await
}
