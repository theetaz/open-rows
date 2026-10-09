// SPDX-License-Identifier: Apache-2.0
//! Explain through the session actor and the real `SQLite` driver: query
//! plan rows become a tree; Explain Analyze falls back to the plan.
use openrows_core::SessionManager;
use openrows_core::explain::{ExplainRequest, explain};
use openrows_core::script::ScriptCanceller;
use openrows_driver_api::Session;
use openrows_explain::{Engine, Hotspot, Mode, Weight};
use openrows_results::ResultStore;
use std::sync::Arc;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sqlite_query_plans_become_a_tree() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("explain.db");
    let manager = SessionManager::new(Arc::new(ResultStore::default()));
    let session = manager
        .open(async move {
            let session: Box<dyn Session> =
                Box::new(openrows_driver_sqlite::open(&path, false, true)?);
            Ok(session)
        })
        .await?;
    for sql in [
        "CREATE TABLE c (id INTEGER PRIMARY KEY, region TEXT)",
        "CREATE TABLE o (id INTEGER PRIMARY KEY, c INT)",
        "CREATE INDEX o_c ON o (c)",
    ] {
        let mut handle = manager.execute(session, sql)?;
        while handle.next_event().await.is_some() {}
    }
    let statement = "SELECT * FROM c JOIN o ON o.c = c.id WHERE c.region = 'eu'";
    for mode in [Mode::Plan, Mode::Analyze] {
        let request = ExplainRequest {
            engine: Engine::Sqlite,
            mode,
            statement,
        };
        let explained = explain(&manager, session, request, &ScriptCanceller::default()).await?;
        assert!(!explained.plan.analyzed && !explained.rolled_back);
        assert_eq!(explained.plan.weight, Weight::None);
        assert!(
            explained.raw.starts_with("QUERY PLAN\n"),
            "{}",
            explained.raw
        );
        let scan = explained
            .plan
            .nodes
            .iter()
            .find(|node| node.data.relation.as_deref() == Some("c"))
            .ok_or("no access to c")?;
        assert!(scan.hotspots.contains(&Hotspot::FullScan { rows: None }));
        let search = explained
            .plan
            .nodes
            .iter()
            .find(|node| node.data.index.as_deref() == Some("o_c"));
        assert!(search.is_some(), "{}", explained.raw);
    }
    Ok(())
}
