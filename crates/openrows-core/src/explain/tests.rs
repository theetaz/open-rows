// SPDX-License-Identifier: Apache-2.0
//! The rollback guarantees of [`explain`] against a recording session.
use super::*;
use crate::BatchSink;
use arrow_array::{Array, RecordBatch, StringArray};
use arrow_schema::SchemaRef;
use openrows_driver_api::{
    BoxFuture, CancelHandle, DriverError, ExecSummary, ResultSink, Session, SinkFlow,
};
use openrows_types::ResultId;
use std::sync::{Mutex, PoisonError};

#[path = "sequence_tests.rs"]
mod sequence_tests;

const PLAN: &str = r#"[{"Plan": {"Node Type": "Result", "Total Cost": 0.01,
    "Actual Total Time": 0.002, "Actual Loops": 1, "Actual Rows": 1, "Plan Rows": 1},
    "Execution Time": 0.01}]"#;

/// Answers `EXPLAIN`, tracks `BEGIN`/`COMMIT`, records every statement and
/// fails statements starting with one of `fail`.
struct Recorder {
    log: Arc<Mutex<Vec<String>>>,
    in_transaction: bool,
    fail: Vec<&'static str>,
    hooks: Arc<CancelHooks>,
}

impl Recorder {
    fn run(&mut self, sql: &str, sink: &mut dyn ResultSink) -> Result<ExecSummary, DriverError> {
        if sql == "BEGIN" {
            self.in_transaction = true;
        }
        self.log
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(sql.to_owned());
        if self.fail.iter().any(|prefix| sql.starts_with(prefix)) {
            return Err(DriverError::Server {
                code: "25P02".to_owned(),
                message: "current transaction is aborted".to_owned(),
            });
        }
        if sql.starts_with("UPDATE") {
            let column: Arc<dyn Array> = Arc::new(StringArray::from(vec![
                "<ShowPlanXML><RelOp PhysicalOp='Table Update' EstimatedTotalSubtreeCost='1'/></ShowPlanXML>",
            ]));
            let batch =
                RecordBatch::try_from_iter([("Microsoft SQL Server 2005 XML Showplan", column)])?;
            sink.begin_result(batch.schema());
            sink.push_batch(batch);
        }
        if sql.starts_with("EXPLAIN") {
            let column: Arc<dyn Array> = Arc::new(StringArray::from(vec![PLAN]));
            let batch = RecordBatch::try_from_iter([("QUERY PLAN", column)])?;
            sink.begin_result(batch.schema());
            sink.push_batch(batch);
        }
        Ok(ExecSummary::default())
    }
}

impl Session for Recorder {
    fn execute<'a>(
        &'a mut self,
        sql: &'a str,
        sink: &'a mut dyn ResultSink,
    ) -> BoxFuture<'a, Result<ExecSummary, DriverError>> {
        Box::pin(async move {
            let summary = self.run(sql, sink)?;
            if sql.starts_with("UPDATE") && self.fail.contains(&"CANCEL") {
                self.hooks.started.notify_one();
                self.hooks.cancelled.notified().await;
                return Err(DriverError::Server {
                    code: "HY008".to_owned(),
                    message: "cancelled".to_owned(),
                });
            }
            Ok(summary)
        })
    }

    fn cancel_handle(&self) -> Option<Arc<dyn CancelHandle>> {
        Some(Arc::clone(&self.hooks) as Arc<dyn CancelHandle>)
    }

    fn in_transaction(&self) -> bool {
        self.in_transaction
    }

    fn close(self: Box<Self>) -> BoxFuture<'static, Result<(), DriverError>> {
        Box::pin(async { Ok(()) })
    }
}

#[derive(Default)]
struct CancelHooks {
    started: tokio::sync::Notify,
    cancelled: tokio::sync::Notify,
}

impl CancelHandle for CancelHooks {
    fn cancel(&self) -> BoxFuture<'static, Result<(), DriverError>> {
        self.cancelled.notify_one();
        Box::pin(async { Ok(()) })
    }
}

struct Discard;

impl BatchSink for Discard {
    fn begin(&self, _result: ResultId, _schema: SchemaRef) {}
    fn push(&self, _result: ResultId, _batch: RecordBatch) -> SinkFlow {
        SinkFlow::Continue
    }
    fn end(&self, _result: ResultId) {}
}

type Run = (Result<Explained, ExplainError>, Vec<String>);

async fn run_engine(
    (engine, mode): (Engine, Mode),
    in_transaction: bool,
    fail: Vec<&'static str>,
) -> Result<Run, Box<dyn std::error::Error>> {
    let manager = SessionManager::new(Arc::new(Discard));
    let log = Arc::new(Mutex::new(Vec::new()));
    let cancelling = fail.contains(&"CANCEL");
    let hooks = Arc::new(CancelHooks::default());
    let recorder = Recorder {
        log: Arc::clone(&log),
        in_transaction,
        fail,
        hooks: Arc::clone(&hooks),
    };
    let session = manager
        .open(async move { Ok(Box::new(recorder) as Box<dyn Session>) })
        .await?;
    if in_transaction {
        let mut begin = manager.execute(session, "BEGIN")?;
        while begin.next_event().await.is_some() {}
    }
    let request = ExplainRequest {
        engine,
        mode,
        statement: "UPDATE t SET a = 1",
    };
    let cancel = ScriptCanceller::default();
    let pending = explain(&manager, session, request, &cancel);
    let result = if cancelling {
        let (result, ()) = tokio::join!(pending, async {
            hooks.started.notified().await;
            assert_eq!(
                manager
                    .execute(session, "SELECT interleaved")
                    .err()
                    .map(|error| error.kind),
                Some(ErrorKind::QueueFull)
            );
            assert!(manager.sequence(session).is_err());
            cancel.cancel();
        });
        result
    } else {
        pending.await
    };
    assert!(manager.sequence(session).is_ok());
    let log = log.lock().unwrap_or_else(PoisonError::into_inner).clone();
    Ok((result, log))
}

async fn run(
    mode: Mode,
    in_transaction: bool,
    fail: Vec<&'static str>,
) -> Result<Run, Box<dyn std::error::Error>> {
    run_engine((Engine::Postgres, mode), in_transaction, fail).await
}

fn kinds(log: &[String]) -> Vec<&str> {
    log.iter()
        .map(|sql| sql.split_whitespace().next().unwrap_or_default())
        .collect()
}

#[tokio::test]
async fn a_plan_runs_only_explain() -> Result<(), Box<dyn std::error::Error>> {
    let (result, log) = run(Mode::Plan, false, Vec::new()).await?;
    let explained = result?;
    assert!(!explained.rolled_back);
    assert_eq!(explained.format, Format::PostgresJson);
    assert_eq!(kinds(&log), ["EXPLAIN"]);
    Ok(())
}

#[tokio::test]
async fn analyze_runs_in_a_transaction_that_is_rolled_back()
-> Result<(), Box<dyn std::error::Error>> {
    let (result, log) = run(Mode::Analyze, false, Vec::new()).await?;
    let explained = result?;
    assert!(explained.rolled_back && explained.plan.analyzed);
    assert_eq!(kinds(&log), ["BEGIN", "EXPLAIN", "ROLLBACK"]);
    Ok(())
}

#[tokio::test]
async fn an_open_transaction_gets_a_savepoint_instead() -> Result<(), Box<dyn std::error::Error>> {
    let (result, log) = run(Mode::Analyze, true, Vec::new()).await?;
    assert!(result?.rolled_back);
    assert_eq!(
        kinds(&log),
        ["BEGIN", "SAVEPOINT", "EXPLAIN", "ROLLBACK", "RELEASE"]
    );
    assert_eq!(log[3], "ROLLBACK TO SAVEPOINT openrows_explain");
    assert!(
        !log.contains(&"ROLLBACK".to_owned()),
        "the user's transaction stays open"
    );
    Ok(())
}

#[tokio::test]
async fn a_failed_statement_is_still_rolled_back() -> Result<(), Box<dyn std::error::Error>> {
    let (result, log) = run(Mode::Analyze, false, vec!["EXPLAIN"]).await?;
    assert!(matches!(result, Err(ExplainError::Failed(_))));
    assert_eq!(kinds(&log), ["BEGIN", "EXPLAIN", "ROLLBACK"]);
    Ok(())
}

#[tokio::test]
async fn a_failed_begin_has_nothing_to_roll_back() -> Result<(), Box<dyn std::error::Error>> {
    let (result, log) = run(Mode::Analyze, true, vec!["SAVEPOINT"]).await?;
    assert!(matches!(result, Err(ExplainError::Failed(_))));
    assert_eq!(kinds(&log), ["BEGIN", "SAVEPOINT"]);
    Ok(())
}

#[tokio::test]
async fn a_failed_rollback_is_reported_as_such() -> Result<(), Box<dyn std::error::Error>> {
    let (result, _) = run(Mode::Analyze, false, vec!["ROLLBACK"]).await?;
    assert!(matches!(result, Err(ExplainError::NotRolledBack(_))));
    Ok(())
}

#[tokio::test]
async fn sqlserver_options_reset_after_statement_and_setup_errors()
-> Result<(), Box<dyn std::error::Error>> {
    for fail in [vec!["UPDATE"], vec!["SET STATISTICS XML ON"]] {
        let (result, log) = run_engine((Engine::SqlServer, Mode::Analyze), false, fail).await?;
        assert!(matches!(result, Err(ExplainError::Failed(_))));
        assert!(log.ends_with(&[
            "SET STATISTICS XML OFF".to_owned(),
            "IF @@TRANCOUNT > 0 ROLLBACK TRAN".to_owned()
        ]));
    }
    let (result, log) = run_engine((Engine::SqlServer, Mode::Plan), false, vec!["UPDATE"]).await?;
    assert!(result.is_err());
    assert_eq!(
        log,
        [
            "SELECT 1",
            "SET SHOWPLAN_XML ON",
            "UPDATE t SET a = 1",
            "SET SHOWPLAN_XML OFF"
        ]
    );
    Ok(())
}

#[tokio::test]
async fn sqlserver_reset_error_still_attempts_rollback() -> Result<(), Box<dyn std::error::Error>> {
    let (result, log) = run_engine(
        (Engine::SqlServer, Mode::Analyze),
        false,
        vec!["SET STATISTICS XML OFF"],
    )
    .await?;
    assert!(matches!(result, Err(ExplainError::NotRestored(_))));
    assert_eq!(
        log.last().map(String::as_str),
        Some("IF @@TRANCOUNT > 0 ROLLBACK TRAN")
    );
    Ok(())
}

#[tokio::test]
async fn sqlserver_cancel_still_resets_options_and_rolls_back()
-> Result<(), Box<dyn std::error::Error>> {
    let (result, log) =
        run_engine((Engine::SqlServer, Mode::Analyze), false, vec!["CANCEL"]).await?;
    assert!(matches!(result, Err(ExplainError::Cancelled)));
    assert!(log.ends_with(&[
        "SET STATISTICS XML OFF".to_owned(),
        "IF @@TRANCOUNT > 0 ROLLBACK TRAN".to_owned()
    ]));
    Ok(())
}
