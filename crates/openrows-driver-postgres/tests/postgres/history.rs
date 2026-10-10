// SPDX-License-Identifier: Apache-2.0
//! Executed `PostgreSQL` statements round-trip through literal-redacted local history.
use super::support::{TestResult, connect, single_batch};
use openrows_sql::{Dialect, has_redacted_literals, redact_literals};
use openrows_store::profile::ProfileId;
use openrows_store::{HistoryRecord, HistorySource, HistoryStatus, LocalStore};
struct Fixture(std::path::PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[tokio::test]
#[ignore = "needs PostgreSQL; run just test-pg"]
async fn executed_history_redacts_strings_numbers_and_dollar_quotes() -> TestResult {
    let mut session = connect().await?;
    let sql = "SELECT 'x'::text, 123::int, $$x$$::text, $tag$x$tag$::text";
    let batch = single_batch(&mut session, sql).await?;
    assert_eq!(batch.num_rows(), 1);
    let root =
        std::env::var("TMPDIR").map_or_else(|_| std::env::temp_dir(), std::path::PathBuf::from);
    let fixture = Fixture(root.join(format!("history-{}", ProfileId::new())));
    std::fs::create_dir_all(&fixture.0)?;
    let path = fixture.0.join("history.sqlite");
    let local = LocalStore::open(&path)?;
    local.record_history(&HistoryRecord {
        profile_id: "test".into(),
        database: Some("postgres".into()),
        started_at_ms: 1,
        duration_ms: None,
        status: HistoryStatus::Ok,
        rows: Some(1),
        statement: redact_literals(sql, Dialect::Postgres),
        error_message: None,
        source: HistorySource::Editor,
    })?;
    drop(local);
    let local = LocalStore::open(&path)?;
    let rows = local.search_history(Some("test"), "", 10)?;
    assert_eq!(rows.len(), 1);
    assert!(has_redacted_literals(&rows[0].record.statement));
    assert!(!rows[0].record.statement.contains("123"));
    assert!(!rows[0].record.statement.contains("'x'"));
    assert!(local.search_history(None, "x", 10)?.is_empty());
    drop(local);
    Ok(())
}
