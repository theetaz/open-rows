// SPDX-License-Identifier: Apache-2.0
//! Real collector evidence crosses the same disclosure and artifact boundaries.
use openrows_ai::{
    AiControl, ProviderConfig, ProviderKind,
    investigation::{self, ArtifactProposal, Ledger},
    sharing::{ContextBundle, GateRequest, SharingLevel, SharingPolicy, SharingStore},
};
use openrows_driver_api::Session;
use openrows_driver_postgres::{PgConfig, PgSession};
use openrows_monitor::{LiveStore, MonitorReader, SampleState, Source};
use openrows_sql::Dialect;
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[tokio::test]
#[ignore = "requires just test-pg's native fixture"]
async fn postgres_monitor_evidence_produces_a_grounded_report() -> TestResult {
    let mut session = PgSession::connect(&PgConfig {
        host: "127.0.0.1".into(),
        port: std::env::var("OPENROWS_TEST_PG_PORT")?.parse()?,
        user: "postgres".into(),
        password: Some(std::env::var("OPENROWS_TEST_PG_PASSWORD")?),
        database: "postgres".into(),
    })
    .await?;
    let descriptors = session.discover().await;
    let mut live = LiveStore::default();
    for source in [Source::Activity, Source::Database] {
        let reading = session.read(source, 1000).await;
        assert_eq!(reading.state, SampleState::Ok);
        live.ingest(source, reading, 0, 1000.0);
    }
    Box::new(session).close().await?;
    let evidence =
        investigation::health("postgres-fixture", &live.snapshot(), &descriptors, 1000.0);
    assert_eq!(evidence.len(), 2);
    let bundle = ContextBundle {
        evidence,
        ..ContextBundle::default()
    };
    grounded_report(bundle)
}

fn grounded_report(bundle: ContextBundle) -> TestResult {
    let sharing = SharingStore::memory(10)?;
    let control = AiControl::default();
    control.set_enabled(true);
    let provider = ProviderConfig {
        id: "fixture".into(),
        name: "Fixture".into(),
        kind: ProviderKind::Ollama,
        base_url: "http://127.0.0.1:1/".into(),
        allow_insecure_remote: false,
        secret_ref: String::new(),
    };
    sharing.save(
        "fixture",
        SharingPolicy {
            level: SharingLevel::SchemaAndStats,
            providers: vec!["fixture".into()],
            telemetry: true,
            ..SharingPolicy::default()
        },
        false,
    )?;
    let payload = sharing.gate(GateRequest {
        connection: "fixture",
        production: false,
        provider: &provider,
        control: &control,
        dialect: Dialect::Postgres,
        bundle,
    })?;
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
