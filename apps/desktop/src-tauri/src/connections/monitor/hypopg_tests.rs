// SPDX-License-Identifier: Apache-2.0
//! Disposable-session tests cover absent, denied and successful hypothetical checks.
use super::*;
use arrow_array::ArrayRef;
use arrow_schema::{DataType, Field, Schema};
use openrows_driver_api::{BoxFuture, DriverError, ExecSummary};
use std::sync::Mutex;

#[derive(Clone, Copy)]
enum Fixture {
    Absent,
    Denied,
    Available,
}
struct FakeSession {
    trace: Arc<Mutex<Vec<String>>>,
    fixture: Fixture,
}
impl Session for FakeSession {
    fn execute<'a>(
        &'a mut self,
        sql: &'a str,
        sink: &'a mut dyn ResultSink,
    ) -> BoxFuture<'a, Result<ExecSummary, DriverError>> {
        Box::pin(async move {
            self.trace
                .lock()
                .map_err(|_| DriverError::Unsupported("fixture".into()))?
                .push(sql.to_owned());
            if sql.contains("hypopg_create_index") && matches!(self.fixture, Fixture::Denied) {
                return Err(DriverError::Unsupported("denied".into()));
            }
            let value = if sql.contains("FROM pg_extension") {
                (!matches!(self.fixture, Fixture::Absent)).then_some("public")
            } else if sql.starts_with("EXPLAIN") {
                let created = self
                    .trace
                    .lock()
                    .map_err(|_| DriverError::Unsupported("fixture".into()))?
                    .iter()
                    .any(|item| item.contains("hypopg_create_index"));
                Some(plan_json(created))
            } else {
                None
            };
            if let Some(value) = value {
                let schema = Arc::new(Schema::new(vec![Field::new(
                    "value",
                    DataType::Utf8,
                    false,
                )]));
                let column: ArrayRef = Arc::new(StringArray::from(vec![value]));
                let batch = RecordBatch::try_new(schema.clone(), vec![column])
                    .map_err(|_| DriverError::Unsupported("fixture".into()))?;
                sink.begin_result(schema);
                sink.push_batch(batch);
            }
            Ok(ExecSummary::default())
        })
    }
    fn close(self: Box<Self>) -> BoxFuture<'static, Result<(), DriverError>> {
        Box::pin(async move {
            self.trace
                .lock()
                .map_err(|_| DriverError::Unsupported("fixture".into()))?
                .push("CLOSE".into());
            Ok(())
        })
    }
}
fn plan_json(created: bool) -> &'static str {
    if created {
        "[{\"Plan\":{\"Total Cost\":10}}]"
    } else {
        "[{\"Plan\":{\"Total Cost\":100}}]"
    }
}
fn request() -> HypoPgRequest {
    HypoPgRequest {
        target: MonitorTarget {
            profile_id: "p".into(),
            database: "db".into(),
        },
        statement_id: "s".into(),
        suggestion_id: "i".into(),
        sql: "SELECT * FROM t WHERE key = 1".into(),
        schema: "public".into(),
        enabled: true,
        confirm_token: None,
    }
}
async fn fixture(
    kind: Fixture,
) -> Result<(Result<HypoPgEstimate, IpcError>, Vec<String>), DriverError> {
    let trace = Arc::new(Mutex::new(Vec::new()));
    let mut session = DiagnosticSession(Box::new(FakeSession {
        trace: trace.clone(),
        fixture: kind,
    }));
    let result = run(
        &mut session,
        &request(),
        "SELECT * FROM t WHERE key = 1",
        "CREATE INDEX ON \"public\".\"t\" (\"key\")",
    )
    .await;
    Box::new(session).close().await?;
    let calls = trace
        .lock()
        .map_err(|_| DriverError::Unsupported("fixture".into()))?
        .clone();
    Ok((result, calls))
}

#[tokio::test]
async fn absent_extension_never_attempts_creation_and_closes_session()
-> Result<(), Box<dyn std::error::Error>> {
    let (result, calls) = fixture(Fixture::Absent).await?;
    assert!(matches!(
        result,
        Err(IpcError {
            kind: IpcErrorKind::Unsupported,
            ..
        })
    ));
    assert!(!calls.iter().any(|sql| sql.contains("hypopg_create_index")));
    assert_eq!(calls.last().map(String::as_str), Some("CLOSE"));
    Ok(())
}

#[tokio::test]
async fn denied_creation_still_resets_and_closes() -> Result<(), Box<dyn std::error::Error>> {
    let (result, calls) = fixture(Fixture::Denied).await?;
    assert!(result.is_err());
    assert!(calls.iter().any(|sql| sql.contains("hypopg_reset")));
    assert_eq!(calls.last().map(String::as_str), Some("CLOSE"));
    Ok(())
}

#[tokio::test]
async fn available_extension_returns_estimates_and_cleans_up()
-> Result<(), Box<dyn std::error::Error>> {
    let (result, calls) = fixture(Fixture::Available).await?;
    let estimate = result?;
    assert!((estimate.baseline_cost - 100.0).abs() < f64::EPSILON);
    assert!((estimate.hypothetical_cost - 10.0).abs() < f64::EPSILON);
    assert!(calls.iter().any(|sql| sql.contains("hypopg_reset")));
    assert_eq!(calls.last().map(String::as_str), Some("CLOSE"));
    assert!(
        !calls
            .iter()
            .any(|sql| sql.contains("CREATE EXTENSION") || sql.starts_with("SET work_mem"))
    );
    Ok(())
}

#[test]
fn only_current_index_rules_can_enter_the_hypothetical_session() {
    let mut card = openrows_monitor::advisor::Suggestion {
        id: "candidate".into(),
        rule_id: "pg.selective-scan".into(),
        version: 1,
        confidence: "medium".into(),
        impact: String::new(),
        trade_offs: String::new(),
        evidence: String::new(),
        evidence_link: String::new(),
        sql: Some("CREATE INDEX CONCURRENTLY ON \"public\".\"t\" (\"key\");".into()),
        statement_id: Some("s".into()),
        collected_ms: 0.0,
    };
    assert_eq!(
        candidate_sql(&card).ok().as_deref(),
        Some("CREATE INDEX ON \"public\".\"t\" (\"key\")")
    );
    card.rule_id = "pg.unused-index".into();
    assert!(candidate_sql(&card).is_err());
}
