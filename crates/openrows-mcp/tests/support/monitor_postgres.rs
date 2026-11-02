// SPDX-License-Identifier: Apache-2.0
//! Real collector evidence crosses the same disclosure and artifact boundaries.
use openrows_ai::{
    AiControl, ProviderConfig, ProviderKind,
    investigation::{self, ArtifactProposal, Ledger, Rejection},
    sharing::{
        ContextBundle, Evidence, GateRequest, GatedPayload, SharingLevel, SharingPolicy,
        SharingStore,
    },
};
use openrows_driver_api::{DiscardSink, Session};
use openrows_driver_postgres::{PgConfig, PgSession};
use openrows_monitor::{
    LiveStore, MonitorReader, SampleState, Source, SourceStatus, advisor::AdvisorInput,
};
use openrows_sql::Dialect;
use std::time::Instant;
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

async fn connect(database: &str) -> TestResult<PgSession> {
    Ok(PgSession::connect(&PgConfig {
        host: "127.0.0.1".into(),
        port: std::env::var("OPENROWS_TEST_PG_PORT")?.parse()?,
        user: "postgres".into(),
        password: Some(std::env::var("OPENROWS_TEST_PG_PASSWORD")?),
        database: database.into(),
    })
    .await?)
}
async fn sql(session: &mut PgSession, text: &str) -> TestResult {
    session.execute(text, &mut DiscardSink).await?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires just test-pg's native fixture"]
async fn postgres_slow_query_and_lock_evidence_ground_cited_findings() -> TestResult {
    let mut admin = connect("postgres").await?;
    let database = format!("openrows_ai_monitor_{}", std::process::id());
    sql(&mut admin, &format!("CREATE DATABASE {database}")).await?;
    let result = investigate(&database).await;
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
async fn investigate(database: &str) -> TestResult {
    let mut writer = connect(database).await?;
    sql(&mut writer, "CREATE EXTENSION pg_stat_statements").await?;
    sql(
        &mut writer,
        "CREATE TABLE ai_monitor_fixture (id int PRIMARY KEY, value text)",
    )
    .await?;
    sql(
        &mut writer,
        "INSERT INTO ai_monitor_fixture VALUES (1, 'test-only-secret')",
    )
    .await?;
    // Interval deltas need a counter baseline for the statement.
    sql(&mut writer, "SELECT pg_sleep(0.25), 'test-only-secret'").await?;
    let mut monitor = connect(database).await?;
    monitor.discover().await;
    let mut store = LiveStore::default();
    let wall = 1_000_000.0;
    store.ingest(
        Source::Statements,
        monitor.read(Source::Statements, 1000).await,
        0,
        wall,
    );
    sql(&mut writer, "SELECT pg_sleep(0.25), 'test-only-secret'").await?;
    let reading = monitor.read(Source::Statements, 1000).await;
    assert_eq!(reading.state, SampleState::Ok);
    store.ingest(Source::Statements, reading, 1000, wall + 1000.0);
    let input = blocked(database, &mut writer, &mut monitor, &mut store).await?;
    Box::new(monitor).close().await?;
    Box::new(writer).close().await?;
    let mut evidence = investigation::queries("pg-fixture", &input);
    evidence.extend(investigation::locks("pg-fixture", &input));
    let slow = evidence
        .iter()
        .find(|point| point.metric == "query:total_ms" && point.value.is_some_and(|ms| ms >= 200.0))
        .ok_or("slow statement interval missing")?
        .clone();
    let lock = evidence
        .iter()
        .find(|point| point.metric == "lock:edge")
        .ok_or("blocking edge missing")?
        .id
        .clone();
    // A focused read: the slow statement's interval and the lock graph. The
    // gate's byte cap may otherwise truncate a large statement list.
    evidence.retain(|point| point.scope == slow.scope || point.metric.starts_with("lock:"));
    cited_findings(evidence, &slow.id, &lock)
}
/// Hold a row lock until the collector observes the waiting session.
async fn blocked(
    database: &str,
    writer: &mut PgSession,
    monitor: &mut PgSession,
    store: &mut LiveStore,
) -> TestResult<AdvisorInput> {
    sql(writer, "BEGIN").await?;
    sql(
        writer,
        "UPDATE ai_monitor_fixture SET value = 'x' WHERE id = 1",
    )
    .await?;
    let mut waiter = connect(database).await?;
    let pending = tokio::spawn(async move {
        sql(
            &mut waiter,
            "UPDATE ai_monitor_fixture SET value = 'y' WHERE id = 1",
        )
        .await
        .map_err(|_| "waiter failed")
    });
    let started = Instant::now();
    let mut input = store.advisor_input(1_002_000.0);
    while input.locks.is_empty() && started.elapsed().as_secs() < 5 {
        let reading = monitor.read(Source::Activity, 1000).await;
        store.ingest(Source::Activity, reading, 2000, 1_002_000.0);
        input = store.advisor_input(1_002_000.0);
    }
    sql(writer, "ROLLBACK").await?;
    pending.await??;
    Ok(input)
}
fn cited_findings(evidence: Vec<Evidence>, slow: &str, lock: &str) -> TestResult {
    let (sharing, provider, control) = fixture()?;
    let refused = gate(&sharing, &provider, &control, evidence.clone());
    assert!(refused.is_err(), "telemetry consent is required");
    sharing.save("fixture", policy(true), false)?;
    let payload = gate(&sharing, &provider, &control, evidence)?;
    let text = payload.preview().payload;
    assert!(!text.contains("test-only-secret") && !text.contains("pg_sleep"));
    let ledger = Ledger::default();
    ledger.record("fixture", &payload)?;
    let delivered = ledger.ids("fixture");
    assert!(delivered.iter().any(|id| id == slow) && delivered.iter().any(|id| id == lock));
    let finding = |ids: Vec<&str>| -> TestResult<ArtifactProposal> {
        Ok(serde_json::from_value(serde_json::json!({
            "kind":"finding", "finding":{"interpretation":"A slow statement waited on a row lock",
            "uncertainty":"One interval and one blocking sample", "evidence":ids}
        }))?)
    };
    let artifact = ledger.propose("fixture", Dialect::Postgres, finding(vec![slow, lock])?)?;
    assert!(artifact.document().contains(slow) && artifact.document().contains(lock));
    let fabricated = "f".repeat(64);
    assert_eq!(
        ledger
            .propose(
                "fixture",
                Dialect::Postgres,
                finding(vec![slow, &fabricated])?
            )
            .err(),
        Some(Rejection::UnknownEvidence)
    );
    Ok(())
}
fn fixture() -> TestResult<(SharingStore, ProviderConfig, AiControl)> {
    let sharing = SharingStore::memory(10)?;
    sharing.save("fixture", policy(false), false)?;
    let control = AiControl::default();
    control.set_enabled(true);
    Ok((sharing, provider(), control))
}
fn policy(telemetry: bool) -> SharingPolicy {
    SharingPolicy {
        level: SharingLevel::SchemaAndStats,
        providers: vec!["fixture".into()],
        telemetry,
        ..SharingPolicy::default()
    }
}
fn provider() -> ProviderConfig {
    ProviderConfig {
        id: "fixture".into(),
        name: "Fixture".into(),
        kind: ProviderKind::Ollama,
        base_url: "http://127.0.0.1:1/".into(),
        allow_insecure_remote: false,
        secret_ref: String::new(),
    }
}
fn gate(
    sharing: &SharingStore,
    provider: &ProviderConfig,
    control: &AiControl,
    evidence: Vec<Evidence>,
) -> Result<GatedPayload, openrows_ai::sharing::Refusal> {
    sharing.gate(GateRequest {
        connection: "fixture",
        production: false,
        provider,
        control,
        dialect: Dialect::Postgres,
        bundle: ContextBundle {
            evidence,
            ..ContextBundle::default()
        },
    })
}

#[tokio::test]
#[ignore = "requires just test-pg's native fixture"]
async fn postgres_monitor_evidence_produces_a_grounded_report() -> TestResult {
    let mut session = connect("postgres").await?;
    let descriptors = session.discover().await;
    let mut statuses = Vec::new();
    for source in [Source::Activity, Source::Database] {
        let reading = session.read(source, 1000).await;
        assert_eq!(reading.state, SampleState::Ok);
        // The collector actor records the same status for every reading.
        statuses.push(SourceStatus {
            source,
            state: reading.state,
            collected_ms: 1000.0,
            last_success_ms: Some(1000.0),
            interval_ms: 1000,
            duration_ms: 1.0,
            sequence: 1,
            partial: reading.partial,
        });
    }
    Box::new(session).close().await?;
    let evidence = investigation::health("postgres-fixture", &statuses, &descriptors, 1000.0);
    assert_eq!(evidence.len(), 2);
    assert!(evidence.iter().all(|point| point.scope != "unknown"));
    grounded_report(evidence)
}

fn grounded_report(evidence: Vec<Evidence>) -> TestResult {
    let (sharing, provider, control) = fixture()?;
    sharing.save("fixture", policy(true), false)?;
    let payload = gate(&sharing, &provider, &control, evidence)?;
    let ledger = Ledger::default();
    ledger.record("fixture", &payload)?;
    let ids = ledger.ids("fixture");
    let proposal: ArtifactProposal = serde_json::from_value(serde_json::json!({
        "kind":"report", "title":"PostgreSQL collector evidence", "chart":false,
        "evidence":ids, "findings":[{"interpretation":"Both sources were collected",
        "uncertainty":"One snapshot cannot establish a trend", "evidence":ids}]
    }))?;
    let artifact = ledger.propose("fixture", Dialect::Postgres, proposal)?;
    let report: serde_json::Value = serde_json::from_str(artifact.document())?;
    assert_eq!(report["schema"], "openrows.monitor-report");
    assert_eq!(
        report["sections"][0]["rows"]
            .as_array()
            .ok_or("missing evidence")?
            .len(),
        2
    );
    Ok(())
}
