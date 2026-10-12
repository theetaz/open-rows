// SPDX-License-Identifier: Apache-2.0
//! Ephemeral fixture uses real modifications/index counters with a controlled maintenance clock.
use super::{
    monitor::{connection, sql},
    monitor_advisor::evidence,
    support::TestResult,
};
use openrows_driver_api::Session;
use openrows_driver_postgres::PgSession;
use openrows_monitor::advisor::advise;
use std::time::Instant;

pub(super) async fn verify(
    writer: &mut PgSession,
    monitor: &mut PgSession,
    database: &str,
) -> TestResult {
    sql(writer, "SELECT pg_stat_reset()").await?;
    sql(
        writer,
        "ALTER TABLE advisor_large SET (autovacuum_enabled = false)",
    )
    .await?;
    sql(writer, "ANALYZE advisor_large").await?;
    let mut work = connection(database, None).await?;
    sql(
        &mut work,
        "UPDATE advisor_large SET payload = 'test-only' WHERE id <= 40000",
    )
    .await?;
    Box::new(work).close().await?;
    let start = Instant::now();
    let mut input = evidence(monitor).await?;
    while !input
        .tables
        .iter()
        .any(|t| t.name == "advisor_large" && t.modifications.is_some_and(|n| n >= 40_000.0))
        && start.elapsed().as_secs() < 5
    {
        input = evidence(monitor).await?;
    }
    let table = input
        .tables
        .iter_mut()
        .find(|t| t.name == "advisor_large")
        .ok_or("table evidence")?;
    assert!(table.modifications.is_some_and(|n| n >= 40_000.0));
    assert!(table.analyze_age_ms.is_some());
    assert!(
        !advise(&input)
            .iter()
            .any(|c| c.rule_id == "pg.stale-statistics" || c.rule_id == "pg.unused-index")
    );
    // Simulate age without sleeping a day/week or mutating PostgreSQL internal statistics files.
    for table in &mut input.tables {
        if table.name == "advisor_large" {
            table.analyze_age_ms = Some(86_400_000.0);
        }
    }
    for index in &mut input.indexes {
        if index.reset_age_ms.is_some() {
            index.reset_age_ms = Some(604_800_000.0);
        }
    }
    let cards = advise(&input);
    assert!(cards.iter().any(|c| c.rule_id == "pg.stale-statistics"
        && c.sql.as_deref() == Some("ANALYZE \"public\".\"advisor_large\";")));
    assert!(
        cards
            .iter()
            .any(|c| c.rule_id == "pg.unused-index" && c.evidence.contains("idx_scan = 0"))
    );
    assert!(
        cards
            .iter()
            .filter(|c| c.rule_id == "pg.unused-index")
            .all(|c| !c.evidence.contains("pkey") && !c.evidence.contains("advisor_unique"))
    );
    Ok(())
}
