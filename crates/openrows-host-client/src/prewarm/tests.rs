// SPDX-License-Identifier: Apache-2.0
use super::*;

#[tokio::test]
async fn attempt_runs_once_and_connect_waits_without_blocking_runtime() {
    let prewarm = Prewarm::default();
    let Some(attempt) = prewarm.start() else {
        unreachable!("new prewarm is pending");
    };
    assert!(prewarm.start().is_none());
    let wait = prewarm.wait();
    tokio::pin!(wait);
    assert!(matches!(futures_poll(&mut wait), std::task::Poll::Pending));
    // The same single-threaded runtime can paint/serve unrelated requests meanwhile.
    let responsive = tokio::spawn(async { 7 });
    assert_eq!(responsive.await.ok(), Some(7));
    let report = attempt.finish(PrewarmOutcome::Ready, Some(Duration::from_millis(9)));
    wait.await;
    assert_eq!(prewarm.report(), Some(report));
    assert!(prewarm.start().is_none());
}

#[tokio::test]
async fn failed_prewarm_is_diagnostic_and_never_becomes_connect_error() {
    let prewarm = Prewarm::default();
    if let Some(attempt) = prewarm.start() {
        attempt.finish(PrewarmOutcome::Failed(HostFailure::Unresponsive), None);
    }
    prewarm.wait().await;
    assert_eq!(
        prewarm.report().map(|report| report.outcome),
        Some(PrewarmOutcome::Failed(HostFailure::Unresponsive))
    );
    assert!(prewarm.start().is_none());
}

#[tokio::test]
async fn before_idle_and_after_cancel_connects_are_unblocked() {
    let prewarm = Prewarm::default();
    prewarm.wait().await;
    let attempt = prewarm.start();
    assert!(attempt.is_some());
    drop(attempt);
    prewarm.wait().await;
    assert_eq!(
        prewarm.report().map(|report| report.outcome),
        Some(PrewarmOutcome::Failed(HostFailure::Exited))
    );
}

#[tokio::test]
async fn duckdb_supervisor_defers_spawn_until_prewarm_finishes()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::Builder::new()
        .prefix(".p-")
        .tempdir_in(if cfg!(unix) {
            std::path::PathBuf::from("/tmp")
        } else {
            std::env::temp_dir()
        })?;
    let prewarm = Prewarm::default();
    let attempt = prewarm.start().ok_or("pending prewarm")?;
    let launcher = crate::HostSupervisor::new(crate::LaunchOptions {
        executable: "missing".into(),
        driver: crate::HostDriverId::Duckdb,
        endpoint_root: directory.path().to_path_buf(),
        memory: crate::MemoryBudget::default(),
        circuit: crate::CircuitBreaker::default(),
    })
    .with_prewarm(prewarm);
    let connect = launcher.spawn();
    tokio::pin!(connect);
    assert!(matches!(
        futures_poll(&mut connect),
        std::task::Poll::Pending
    ));
    assert_eq!(std::fs::read_dir(directory.path())?.count(), 0);
    attempt.finish(PrewarmOutcome::Failed(HostFailure::Incompatible), None);
    // Connect performs its own launch validation rather than inheriting the prewarm error.
    assert!(matches!(connect.await, Err(HostFailure::Spawn)));
    Ok(())
}

#[tokio::test]
async fn missing_installed_helper_is_skipped_once_without_endpoints()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let endpoints = directory.path().join("hosts");
    let runtime = crate::DriverHostRuntime::new(
        directory.path().join("missing/openrows-app"),
        endpoints.clone(),
    );
    let report = runtime.prewarm().await.ok_or("missing prewarm report")?;
    assert_eq!(report.outcome, PrewarmOutcome::Skipped(HostFailure::Spawn));
    assert_eq!(runtime.prewarm_report(), Some(report));
    assert!(runtime.prewarm().await.is_none());
    assert!(!endpoints.exists());
    Ok(())
}

fn futures_poll<F: Future>(future: &mut std::pin::Pin<&mut F>) -> std::task::Poll<F::Output> {
    future
        .as_mut()
        .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()))
}
