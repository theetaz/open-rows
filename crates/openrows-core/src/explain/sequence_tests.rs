// SPDX-License-Identifier: Apache-2.0
//! Reservations prevent option leakage and refresh queued transaction state.
use super::*;

#[tokio::test]
async fn sqlserver_drains_queued_begin_before_choosing_savepoint()
-> Result<(), Box<dyn std::error::Error>> {
    let manager = SessionManager::new(Arc::new(Discard));
    let log = Arc::new(Mutex::new(Vec::new()));
    let recorder = Recorder {
        log: Arc::clone(&log),
        in_transaction: false,
        fail: Vec::new(),
        hooks: Arc::new(CancelHooks::default()),
    };
    let session = manager
        .open(async move { Ok(Box::new(recorder) as Box<dyn Session>) })
        .await?;
    let _queued = manager.execute(session, "BEGIN")?;
    let plan = explain(
        &manager,
        session,
        ExplainRequest {
            engine: Engine::SqlServer,
            mode: Mode::Analyze,
            statement: "UPDATE t SET a = 1",
        },
        &ScriptCanceller::default(),
    )
    .await?;
    assert!(plan.rolled_back);
    let log = log.lock().unwrap_or_else(PoisonError::into_inner);
    assert_eq!(
        &log[..3],
        ["BEGIN", "SELECT 1", "SAVE TRAN openrows_explain"]
    );
    assert!(
        !log.iter()
            .any(|sql| sql == "IF @@TRANCOUNT > 0 ROLLBACK TRAN")
    );
    Ok(())
}
