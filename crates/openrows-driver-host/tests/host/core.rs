// SPDX-License-Identifier: Apache-2.0
//! A host failure has one terminal owner and cannot disturb independent actors.
use crate::support;
use openrows_core::{BatchSink, ErrorKind, QueryEvent, QueryHandle, SessionManager, SessionState};
use openrows_driver_api::{ConnectTrace, Driver, Session, SinkAdmission, SinkFlow};
use openrows_host_client::{HostSupervisor, HostedConnection, HostedDriver, MemoryBudget};
use std::{sync::Arc, time::Duration};

pub(super) async fn hosted() -> HostedConnection {
    HostedDriver::new(
        HostSupervisor::new(support::options(MemoryBudget::default())),
        Arc::new(openrows_driver_sqlite::SqliteDriver),
    )
    .open(
        &support::config().to_request().unwrap(),
        &mut ConnectTrace::default(),
    )
    .await
    .unwrap()
}
#[cfg(unix)]
fn kill(pid: u32) {
    rustix::process::kill_process(
        rustix::process::Pid::from_raw(i32::try_from(pid).unwrap()).unwrap(),
        rustix::process::Signal::KILL,
    )
    .unwrap();
}
pub(super) async fn events(mut query: QueryHandle) -> Vec<QueryEvent> {
    tokio::time::timeout(Duration::from_secs(8), async move {
        let mut events = Vec::new();
        while let Some(event) = query.next_event().await {
            events.push(event);
        }
        events
    })
    .await
    .unwrap()
}
fn failed_once(events: &[QueryEvent]) {
    assert_eq!(events.iter().filter(|event| matches!(event, QueryEvent::Failed { error } if error.kind == ErrorKind::DriverHost)).count(), 1);
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, QueryEvent::Completed { .. } | QueryEvent::Cancelled))
            .count(),
        0
    );
}
#[cfg(unix)]
#[tokio::test]
async fn idle_host_loss_fails_only_a_while_hosted_b_and_native_c_keep_querying() {
    let _permit = support::host_test().await.unwrap();
    let manager = SessionManager::new(Arc::new(openrows_results::ResultStore::default()));
    let mut changes = manager.subscribe();
    let a = hosted().await;
    let pid = a.process_id();
    let a = manager
        .open(async move { Ok(Box::new(a) as Box<dyn Session>) })
        .await
        .unwrap();
    let b = hosted().await;
    let b = manager
        .open(async move { Ok(Box::new(b) as Box<dyn Session>) })
        .await
        .unwrap();
    let native = openrows_driver_sqlite::SqliteDriver
        .connect(
            &support::config().to_request().unwrap(),
            &mut ConnectTrace::default(),
        )
        .await
        .unwrap();
    let c = manager
        .open(async move { Ok(native as Box<dyn Session>) })
        .await
        .unwrap();
    kill(pid);
    let event = reaches(&mut changes, a).await;
    assert_eq!(event.error.unwrap().kind, ErrorKind::DriverHost);
    assert_eq!(manager.state(a), Some(SessionState::Failed));
    for session in [b, c] {
        let events = events(manager.execute(session, "SELECT 1").unwrap()).await;
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, QueryEvent::Completed { .. }))
                .count(),
            1
        );
        assert_eq!(manager.state(session), Some(SessionState::Idle));
        manager.close(session).await.unwrap();
    }
    manager.close(a).await.unwrap();
}
struct FullSink {
    store: openrows_results::ResultStore,
    pushed: tokio::sync::Notify,
}
impl BatchSink for FullSink {
    fn begin(&self, result: openrows_types::ResultId, schema: arrow_schema::SchemaRef) {
        self.store.begin(&result.to_string(), schema);
    }
    fn push(&self, result: openrows_types::ResultId, batch: arrow_array::RecordBatch) -> SinkFlow {
        let _ = self.store.append(&result.to_string(), batch);
        self.pushed.notify_one();
        SinkFlow::Continue
    }
    fn end(&self, _result: openrows_types::ResultId) {}
    fn admission(&self, _result: openrows_types::ResultId) -> SinkAdmission {
        SinkAdmission::Pause
    }
    fn resume(
        &self,
        _result: openrows_types::ResultId,
    ) -> openrows_driver_api::BoxFuture<'_, SinkAdmission> {
        Box::pin(std::future::pending())
    }
}
#[cfg(unix)]
#[tokio::test]
async fn busy_host_loss_preserves_partial_result_and_completes_every_queued_caller_once() {
    let _permit = support::host_test().await.unwrap();
    let sink = Arc::new(FullSink {
        store: openrows_results::ResultStore::default(),
        pushed: tokio::sync::Notify::new(),
    });
    let manager = SessionManager::new(sink.clone());
    let connection = hosted().await;
    let pid = connection.process_id();
    let session = manager
        .open(async move { Ok(Box::new(connection) as Box<dyn Session>) })
        .await
        .unwrap();
    let active = manager.execute(session, "WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<100000) SELECT x FROM n").unwrap();
    tokio::time::timeout(Duration::from_secs(5), sink.pushed.notified())
        .await
        .unwrap();
    let queued: Vec<_> = (0..8)
        .map(|_| manager.execute(session, "SELECT 2").unwrap())
        .collect();
    kill(pid);
    let active = events(active).await;
    failed_once(&active);
    let result = active
        .iter()
        .find_map(|e| {
            if let QueryEvent::ResultBegin { result, .. } = e {
                Some(result)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(sink.store.rows(&result.to_string()), Some((200, false)));
    assert_eq!(
        active
            .iter()
            .filter(|e| matches!(e, QueryEvent::ResultEnd { .. }))
            .count(),
        1
    );
    for query in queued {
        failed_once(&events(query).await);
    }
    assert_eq!(manager.state(session), Some(SessionState::Failed));
    manager.close(session).await.unwrap();
}
#[tokio::test]
async fn noncooperative_adapter_cancel_is_reaped_after_grace() {
    let _permit = support::host_test().await.unwrap();
    let manager = SessionManager::new(Arc::new(openrows_results::ResultStore::default()));
    let connection = hosted().await;
    let failure = connection.host_failure().unwrap();
    let session = manager
        .open(async move { Ok(Box::new(connection) as Box<dyn Session>) })
        .await
        .unwrap();
    let mut query = manager
        .execute(session, "OPENROWS_TEST_HANG_NOTICE")
        .unwrap();
    assert!(matches!(
        query.next_event().await,
        Some(QueryEvent::Started { .. })
    ));
    assert!(matches!(
        query.next_event().await,
        Some(QueryEvent::Notice { .. })
    ));
    let started = std::time::Instant::now();
    query.canceller().cancel();
    let outcome = events(query).await;
    failed_once(&outcome);
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(4), failure)
            .await
            .unwrap(),
        openrows_driver_api::DriverError::Host(_)
    ));
    assert!(started.elapsed() >= Duration::from_secs(2));
    let _ = std::io::Write::write_fmt(
        &mut std::io::stderr(),
        format_args!(
            "noncooperative adapter cancel/reap: {:.3} ms\n",
            started.elapsed().as_secs_f64() * 1000.0
        ),
    );
    manager.close(session).await.unwrap();
}

async fn reaches(
    changes: &mut tokio::sync::broadcast::Receiver<openrows_core::SessionEvent>,
    session: openrows_types::SessionId,
) -> openrows_core::SessionEvent {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let event = changes.recv().await.unwrap();
            if event.session == session && event.state == SessionState::Failed {
                return event;
            }
        }
    })
    .await
    .unwrap()
}
#[tokio::test]
async fn resilient_host_loss_never_invokes_reconnect_or_replays_setup() {
    use std::sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    };
    let _permit = support::host_test().await.unwrap();
    let connection = Arc::new(Mutex::new(Some(hosted().await)));
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let reconnect = Arc::new(move || {
        observed.fetch_add(1, Ordering::SeqCst);
        let connection = connection.lock().unwrap().take();
        Box::pin(async move {
            connection.map(|c| Box::new(c) as Box<dyn Session>).ok_or(
                openrows_driver_api::DriverError::Host(openrows_host_client::HostFailure::Exited),
            )
        }) as openrows_driver_api::BoxFuture<'static, _>
    });
    let manager = SessionManager::new(Arc::new(openrows_results::ResultStore::default()));
    let session = manager
        .open_resilient(openrows_core::Resilience {
            reconnect,
            dialect: Some(openrows_sql::Dialect::Sqlite),
            policy: openrows_core::RetryPolicy::default(),
            setup: vec!["CREATE TABLE setup(id integer)".into()],
        })
        .await
        .unwrap();
    manager.suspend_idle(session, Duration::ZERO).unwrap();
    failed_once(&events(manager.execute(session, "OPENROWS_TEST_ABORT").unwrap()).await);
    assert_eq!(manager.state(session), Some(SessionState::Failed));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(manager.execute(session, "SELECT 1").is_err());
    manager.close(session).await.unwrap();
}
