// SPDX-License-Identifier: Apache-2.0
//! Real `PostgreSQL` plans/counters exercise deterministic editor-only advice.
use super::{
    monitor::{connection, sql},
    monitor_capture::wall_ms,
    support::{Collect, TestResult},
};
use arrow_array::StringArray;
use openrows_driver_api::Session;
use openrows_driver_postgres::PgSession;
use openrows_explain::{Format, analyse, parse};
use openrows_monitor::{
    LiveStore, MonitorReader, SampleState, Source,
    advisor::{AdvisorInput, PlanEvidence, advise, project_plan},
};

#[tokio::test]
#[ignore = "needs the disposable PostgreSQL container (just test-pg)"]
async fn advisor_real_selective_scan_index_removal_spill_maintenance_and_negatives() -> TestResult {
    let mut admin = super::support::connect().await?;
    let database = format!("openrows_advisor_test_{}", std::process::id());
    sql(&mut admin, &format!("CREATE DATABASE {database}")).await?;
    let result = exercise(&database).await;
    sql(
        &mut admin,
        &format!(
            "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = '{database}'"
        ),
    )
    .await?;
    sql(&mut admin, &format!("DROP DATABASE {database}")).await?;
    result
}
async fn exercise(database: &str) -> TestResult {
    let mut writer = connection(database, None).await?;
    sql(&mut writer, "CREATE EXTENSION pg_stat_statements; CREATE TABLE advisor_large (id int PRIMARY KEY, key int, payload text); INSERT INTO advisor_large SELECT i, i, repeat('x', 100) FROM generate_series(1, 200000) i; CREATE TABLE advisor_small AS SELECT * FROM advisor_large LIMIT 20; ANALYZE advisor_large; ANALYZE advisor_small; CREATE INDEX advisor_unused ON advisor_large(payload); CREATE INDEX advisor_duplicate ON advisor_large(payload); CREATE UNIQUE INDEX advisor_unique ON advisor_large(id)").await?;
    Box::new(writer).close().await?;
    let mut writer = connection(database, None).await?;
    let mut monitor = connection(database, None).await?;
    monitor.discover().await;
    let input = evidence(&mut monitor).await?;
    selective(&mut writer, &mut monitor, input).await?;
    spilling(&mut writer, &mut monitor).await?;
    super::monitor_advisor_age::verify(&mut writer, &mut monitor, database).await?;
    sql(&mut writer, "CREATE TABLE monitor_fixture (id int PRIMARY KEY, value text); INSERT INTO monitor_fixture VALUES (1, 'test-only')").await?;
    super::monitor_blocking::verify(&mut writer, &mut monitor, database).await?;
    Ok(())
}
pub(super) async fn evidence(monitor: &mut PgSession) -> TestResult<AdvisorInput> {
    let mut store = LiveStore::default();
    let wall = wall_ms();
    for source in [Source::Tables, Source::Indexes] {
        let sample = monitor.read(source, 1_000).await;
        assert_eq!(sample.state, SampleState::Ok);
        store.ingest(source, sample, 0, wall);
    }
    Ok(store.advisor_input(wall))
}
async fn plan(
    writer: &mut PgSession,
    query: &str,
    input: &AdvisorInput,
    measured: bool,
) -> TestResult<PlanEvidence> {
    let command = if measured {
        "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)"
    } else {
        "EXPLAIN (FORMAT JSON)"
    };
    let mut sink = Collect::default();
    writer
        .execute(&format!("{command} {query}"), &mut sink)
        .await?;
    let batch = sink.batches.first().ok_or("plan batch")?;
    let text = batch
        .column(0)
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or("plan JSON")?
        .value(0);
    let parsed = analyse(parse(Format::PostgresJson, &[vec![Some(text.into())]])?);
    Ok(project_plan(
        &parsed,
        ("fixture-query", input.now_ms),
        &input.tables,
    ))
}
async fn selective(
    writer: &mut PgSession,
    monitor: &mut PgSession,
    mut input: AdvisorInput,
) -> TestResult {
    input.plans = vec![
        plan(
            writer,
            "SELECT * FROM advisor_large WHERE key = 12345",
            &input,
            false,
        )
        .await?,
    ];
    let cards = advise(&input);
    let index = cards
        .iter()
        .find(|card| card.rule_id == "pg.selective-scan")
        .ok_or("selective scan suggestion")?;
    assert_eq!(
        index.sql.as_deref(),
        Some("CREATE INDEX CONCURRENTLY ON \"public\".\"advisor_large\" (\"key\");")
    );
    assert!(
        index.evidence.contains("200000")
            && index.evidence.contains("estimated")
            && index.evidence.contains("bytes")
    );
    // Fixture mutation is explicit; the advisor itself has no execution handle.
    sql(writer, index.sql.as_deref().ok_or("proposal SQL")?).await?;
    let mut after = evidence(monitor).await?;
    after.plans = input.plans.clone();
    assert!(
        !advise(&after)
            .iter()
            .any(|card| card.rule_id == "pg.selective-scan")
    );
    after.plans = vec![
        plan(
            writer,
            "SELECT * FROM advisor_large WHERE key = 12345",
            &after,
            false,
        )
        .await?,
    ];
    assert!(
        !advise(&after)
            .iter()
            .any(|card| card.rule_id == "pg.selective-scan")
    );
    for query in [
        "SELECT * FROM advisor_small WHERE key = 1",
        "SELECT * FROM advisor_large WHERE id = 1",
        "SELECT * FROM advisor_large WHERE payload = repeat('x', 100)",
    ] {
        after.plans = vec![plan(writer, query, &after, false).await?];
        assert!(
            !advise(&after)
                .iter()
                .any(|card| card.rule_id == "pg.selective-scan")
        );
    }
    assert!(
        advise(&after)
            .iter()
            .any(|card| card.rule_id == "pg.duplicate-index")
    );
    assert!(
        advise(&after)
            .iter()
            .filter(|card| card.sql.as_ref().is_some_and(|sql| sql.starts_with("DROP")))
            .all(|card| {
                !card.evidence.contains("pkey") && !card.evidence.contains("advisor_unique")
            })
    );
    Ok(())
}
async fn spilling(writer: &mut PgSession, monitor: &mut PgSession) -> TestResult {
    // Only the fixture's explicit user session changes memory; the collector never does.
    sql(
        writer,
        "SET work_mem = '64kB'; SET max_parallel_workers_per_gather = 0",
    )
    .await?;
    let mut input = evidence(monitor).await?;
    input.plans = vec![
        plan(
            writer,
            "SELECT * FROM advisor_large ORDER BY key + id",
            &input,
            true,
        )
        .await?,
    ];
    let cards = advise(&input);
    let spill = cards
        .iter()
        .find(|card| card.rule_id == "pg.plan-spill")
        .ok_or("sort spill evidence")?;
    assert_eq!(spill.confidence, "high");
    assert!(spill.sql.is_none());
    assert!(spill.trade_offs.contains("work_mem"));
    Ok(())
}
