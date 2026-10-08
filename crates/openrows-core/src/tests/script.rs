// SPDX-License-Identifier: Apache-2.0
//! Statement selection and sequential multi-statement runs.
use super::{FakeCancel, TestResult, manager, open};
use crate::script::{
    Scope, Script, ScriptCanceller, ScriptEvent, ScriptOutcome, run_bound, run_script, statements,
};
use crate::{QueryEvent, SessionManager};
use openrows_sql::Dialect;
use openrows_sql::select::BoundSql;
use openrows_types::SessionId;
use std::sync::Arc;
use std::time::Duration;

fn owned(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

async fn run(
    manager: &SessionManager,
    session: SessionId,
    items: &[&str],
    cancel: &ScriptCanceller,
) -> (ScriptOutcome, Vec<ScriptEvent>) {
    let mut seen = Vec::new();
    let statements = owned(items);
    let script = Script {
        statements: &statements,
        max_rows: None,
    };
    let outcome = run_script(manager, session, script, cancel, |event| {
        seen.push(event);
    })
    .await;
    (outcome, seen)
}

fn statements_seen(seen: &[ScriptEvent]) -> Vec<usize> {
    let mut indices: Vec<usize> = seen.iter().map(|event| event.statement).collect();
    indices.dedup();
    indices
}

#[test]
fn scope_selects_all_or_the_statement_at_the_cursor() {
    let sql = "SELECT 1;\n-- two\nSELECT 2;  \n\nSELECT 3";
    assert_eq!(
        statements(sql, Scope::All, Dialect::Postgres),
        owned(&["SELECT 1", "-- two\nSELECT 2", "SELECT 3"])
    );
    assert_eq!(
        statements(sql, Scope::AtCursor(0), Dialect::Postgres),
        owned(&["SELECT 1"])
    );
    let second = sql.find("SELECT 2").unwrap_or_default() + 3;
    assert_eq!(
        statements(sql, Scope::AtCursor(second), Dialect::Postgres),
        owned(&["-- two\nSELECT 2"])
    );
    let after_semicolon = sql.find("SELECT 2;").unwrap_or_default() + 10;
    assert_eq!(
        statements(sql, Scope::AtCursor(after_semicolon), Dialect::Postgres),
        owned(&["-- two\nSELECT 2"])
    );
    assert_eq!(
        statements(sql, Scope::AtCursor(sql.len()), Dialect::Postgres),
        owned(&["SELECT 3"])
    );
    assert!(statements("  -- only a comment\n", Scope::All, Dialect::Postgres).is_empty());
    assert!(statements("", Scope::AtCursor(0), Dialect::Postgres).is_empty());
}

#[tokio::test]
async fn statements_run_in_order_with_tagged_events() -> TestResult {
    let (manager, store) = manager();
    let session = open(&manager, None).await?;
    let cancel = ScriptCanceller::default();
    let (outcome, seen) = run(
        &manager,
        session,
        &["rows 1 2", "update", "rows 2 1"],
        &cancel,
    )
    .await;
    assert_eq!(outcome, ScriptOutcome::Completed);
    assert_eq!(statements_seen(&seen), vec![0, 1, 2]);
    assert!(seen.iter().any(|event| event.statement == 1
        && matches!(event.event, QueryEvent::RowsAffected { count: 3 })));
    let completed = seen
        .iter()
        .filter(|event| matches!(event.event, QueryEvent::Completed { .. }))
        .count();
    assert_eq!(completed, 3);
    assert_eq!(store.batches(), 3);
    Ok(())
}

#[tokio::test]
async fn the_first_error_stops_the_run() -> TestResult {
    let (manager, store) = manager();
    let session = open(&manager, None).await?;
    let cancel = ScriptCanceller::default();
    let (outcome, seen) = run(
        &manager,
        session,
        &["rows 1 1", "fail", "rows 1 1", "update"],
        &cancel,
    )
    .await;
    assert_eq!(outcome, ScriptOutcome::Failed);
    assert_eq!(statements_seen(&seen), vec![0, 1]);
    assert!(matches!(
        seen.last(),
        Some(ScriptEvent {
            statement: 1,
            event: QueryEvent::Failed { .. }
        })
    ));
    assert_eq!(store.batches(), 1);
    Ok(())
}

#[tokio::test]
async fn cancel_stops_the_running_statement_and_skips_the_rest() -> TestResult {
    let (manager, store) = manager();
    let session = open(&manager, Some(Arc::new(FakeCancel::default()))).await?;
    let cancel = ScriptCanceller::default();
    let remote = cancel.clone();
    let canceller = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        remote.cancel();
    });
    let (outcome, seen) = run(&manager, session, &["hang", "rows 1 1"], &cancel).await;
    canceller.await?;
    assert_eq!(outcome, ScriptOutcome::Cancelled);
    assert_eq!(statements_seen(&seen), vec![0]);
    assert!(matches!(
        seen.last().map(|event| &event.event),
        Some(QueryEvent::Cancelled)
    ));
    assert_eq!(store.batches(), 0);
    Ok(())
}

#[tokio::test]
async fn a_cancelled_run_never_starts() -> TestResult {
    let (manager, store) = manager();
    let session = open(&manager, None).await?;
    let cancel = ScriptCanceller::default();
    cancel.cancel();
    let (outcome, seen) = run(&manager, session, &["rows 1 1"], &cancel).await;
    assert_eq!(outcome, ScriptOutcome::Cancelled);
    assert_eq!(seen.len(), 1);
    assert_eq!(store.batches(), 0);
    Ok(())
}

#[tokio::test]
async fn an_unknown_session_fails_the_run() -> TestResult {
    let (manager, _) = manager();
    let cancel = ScriptCanceller::default();
    let (outcome, seen) = run(&manager, SessionId::new(), &["rows 1 1"], &cancel).await;
    assert_eq!(outcome, ScriptOutcome::Failed);
    assert!(matches!(
        seen.as_slice(),
        [ScriptEvent {
            statement: 0,
            event: QueryEvent::Failed { .. }
        }]
    ));
    Ok(())
}

#[tokio::test]
async fn bound_statements_reach_the_session_with_their_parameters() -> TestResult {
    let (manager, _store) = manager();
    let session = open(&manager, None).await?;
    let cancel = ScriptCanceller::default();
    let mut seen = Vec::new();
    let plain = BoundSql {
        sql: "rows 1 3".to_owned(),
        params: Vec::new(),
    };
    let outcome = run_bound(&manager, session, plain, &cancel, |event| seen.push(event)).await;
    assert_eq!(outcome, ScriptOutcome::Completed);
    assert!(seen.iter().all(|event| event.statement == 0));
    let bound = BoundSql {
        sql: "rows 1 3".to_owned(),
        params: vec![Some("x".to_owned())],
    };
    let refused = run_bound(&manager, session, bound.clone(), &cancel, |_| {}).await;
    assert_eq!(refused, ScriptOutcome::Failed, "the fake cannot bind");
    cancel.cancel();
    let skipped = run_bound(&manager, session, bound, &cancel, |_| {}).await;
    assert_eq!(skipped, ScriptOutcome::Cancelled);
    Ok(())
}
