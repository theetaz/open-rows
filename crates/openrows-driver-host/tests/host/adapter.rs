// SPDX-License-Identifier: Apache-2.0
//! Adapter admission, facet and explicit reconnect contracts with disposable hosts.
use crate::support;
use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use openrows_driver_api::{
    BoxFuture, ConnectTrace, Connection, Driver, DriverError, ResultSink, SinkAdmission, SinkFlow,
};
use openrows_host_client::{HostSupervisor, HostedDriver, MemoryBudget};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

pub(super) async fn connection() -> Box<dyn Connection> {
    let request = support::config().to_request().unwrap();
    HostedDriver::new(
        HostSupervisor::new(support::options(MemoryBudget::default())),
        Arc::new(openrows_driver_sqlite::SqliteDriver),
    )
    .connect(&request, &mut ConnectTrace::default())
    .await
    .unwrap()
}
#[derive(Default)]
pub(super) struct Pressure {
    pub(super) rows: AtomicUsize,
    pub(super) paused: AtomicBool,
    pub(super) pushed: tokio::sync::Notify,
    pub(super) resume: tokio::sync::Notify,
}
struct Sink(Arc<Pressure>);
impl ResultSink for Sink {
    fn begin_result(&mut self, _schema: SchemaRef) {}
    fn push_batch(&mut self, batch: RecordBatch) -> SinkFlow {
        self.0.rows.fetch_add(batch.num_rows(), Ordering::SeqCst);
        self.0.paused.store(true, Ordering::SeqCst);
        self.0.pushed.notify_one();
        SinkFlow::Continue
    }
    fn admission(&self) -> SinkAdmission {
        if self.0.paused.load(Ordering::SeqCst) {
            SinkAdmission::Pause
        } else {
            SinkAdmission::Ready
        }
    }
    fn resume(&mut self) -> BoxFuture<'_, SinkAdmission> {
        Box::pin(async move {
            while self.0.paused.load(Ordering::SeqCst) {
                self.0.resume.notified().await;
            }
            SinkAdmission::Ready
        })
    }
}
const SCAN: &str =
    "WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<10000) SELECT x FROM n";
#[tokio::test]
async fn paused_adapter_cancel_and_next_query() {
    let _permit = support::host_test().await.unwrap();
    let mut connection = connection().await;
    let cancel = connection.cancel_handle().unwrap();
    let pressure = Arc::new(Pressure::default());
    let mut sink = Sink(pressure.clone());
    let mut execution = connection.execute(SCAN, &mut sink);
    tokio::select! { () = pressure.pushed.notified() => {}, result = &mut execution => panic!("early completion {result:?}") }
    let before = pressure.rows.load(Ordering::SeqCst);
    let started = std::time::Instant::now();
    cancel.cancel().await.unwrap();
    let error = tokio::time::timeout(Duration::from_millis(200), execution)
        .await
        .unwrap()
        .unwrap_err();
    assert!(matches!(error, DriverError::Server { ref code, .. } if code == "57014"));
    let _ = std::io::Write::write_fmt(
        &mut std::io::stderr(),
        format_args!(
            "paused adapter cancel/drain: {:.3} ms; admitted rows {before}\n",
            started.elapsed().as_secs_f64() * 1000.0
        ),
    );
    connection.ping().await.unwrap();
    connection.close().await.unwrap();
}
#[tokio::test]
async fn pause_resume_streams_without_early_credit_or_truncation() {
    let _permit = support::host_test().await.unwrap();
    let mut connection = connection().await;
    let pressure = Arc::new(Pressure::default());
    let mut sink = Sink(pressure.clone());
    let mut execution = connection.execute(SCAN, &mut sink);
    loop {
        tokio::select! {
            result = &mut execution => { assert_eq!(result.unwrap().rows, 10000); break; },
            () = pressure.pushed.notified() => {
                pressure.paused.store(false, Ordering::SeqCst);
                pressure.resume.notify_one();
            }
        }
    }
    assert_eq!(pressure.rows.load(Ordering::SeqCst), 10000);
    drop(execution);
    connection.close().await.unwrap();
}
#[tokio::test]
async fn facets_fail_typed_after_host_loss_and_fresh_connect_works() {
    let _permit = support::host_test().await.unwrap();
    let mut connection = connection().await;
    let error = connection
        .execute("OPENROWS_TEST_ABORT", &mut openrows_driver_api::DiscardSink)
        .await
        .unwrap_err();
    assert!(matches!(error, DriverError::Host(_)));
    assert!(!error.is_connection_lost());
    assert!(matches!(
        connection.catalog().unwrap().list_schemas().await,
        Err(DriverError::Host(_))
    ));
    assert!(matches!(
        connection
            .editor()
            .unwrap()
            .commit_ddl(&["CREATE TABLE x(id integer)".into()])
            .await,
        Err(DriverError::Host(_))
    ));
    drop(connection);
    let mut fresh = self::connection().await;
    fresh.ping().await.unwrap();
    fresh.close().await.unwrap();
}
#[tokio::test]
async fn hosted_transaction_state_tracks_native_begin_and_rollback() {
    let _permit = support::host_test().await.unwrap();
    let mut connection = connection().await;
    connection
        .execute("BEGIN", &mut openrows_driver_api::DiscardSink)
        .await
        .unwrap();
    assert!(connection.in_transaction());
    connection
        .execute("ROLLBACK", &mut openrows_driver_api::DiscardSink)
        .await
        .unwrap();
    assert!(!connection.in_transaction());
    connection.close().await.unwrap();
}

#[tokio::test]
async fn startup_transaction_state_is_published_before_connected_reply() {
    let _permit = support::host_test().await.unwrap();
    let mut config = support::config();
    config.startup = vec!["BEGIN".into()];
    let request = config.to_request().unwrap();
    let mut connection = HostedDriver::new(
        HostSupervisor::new(support::options(MemoryBudget::default())),
        Arc::new(openrows_driver_sqlite::SqliteDriver),
    )
    .connect(&request, &mut ConnectTrace::default())
    .await
    .unwrap();
    assert!(connection.in_transaction());
    connection
        .execute("ROLLBACK", &mut openrows_driver_api::DiscardSink)
        .await
        .unwrap();
    assert!(!connection.in_transaction());
    connection.close().await.unwrap();
}

#[tokio::test]
async fn protocol_error_poisons_immediately_and_publishes_reaped_lifecycle_failure() {
    let _permit = support::host_test().await.unwrap();
    let mut connection = connection().await;
    let failure = connection.host_failure().unwrap();
    assert!(matches!(
        connection
            .execute(
                "OPENROWS_TEST_PROTOCOL_ERROR",
                &mut openrows_driver_api::DiscardSink
            )
            .await,
        Err(DriverError::Host(
            openrows_host_client::HostFailure::Protocol
        ))
    ));
    assert!(matches!(
        connection.ping().await,
        Err(DriverError::Host(
            openrows_host_client::HostFailure::Protocol
        ))
    ));
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(2), failure)
            .await
            .unwrap(),
        DriverError::Host(openrows_host_client::HostFailure::Protocol)
    ));
}

#[tokio::test]
async fn failed_connect_preserves_native_trace_and_does_not_trip_the_crash_circuit() {
    let _permit = support::host_test().await.unwrap();
    let mut config = support::config();
    config.startup = vec!["SELECT * FROM test_only_missing_table".into()];
    let request = config.to_request().unwrap();
    let driver = HostedDriver::new(
        HostSupervisor::new(support::options(MemoryBudget::default())),
        Arc::new(openrows_driver_sqlite::SqliteDriver),
    );
    for _ in 0..4 {
        let mut trace = ConnectTrace::default();
        let result = driver.open(&request, &mut trace).await;
        assert!(matches!(result, Err(DriverError::Server { .. })));
        assert_eq!(
            trace.failed,
            Some(openrows_driver_api::ConnectStage::StartupCommands)
        );
        assert!(!trace.completed.is_empty());
    }
}

#[tokio::test]
async fn failed_file_open_preserves_native_trace_and_tls_status() {
    let _permit = support::host_test().await.unwrap();
    let dir = tempfile::tempdir_in(".").unwrap();
    let path = std::fs::canonicalize(dir.path())
        .unwrap()
        .join("missing.sqlite");
    let mut config = support::config();
    config.endpoint = openrows_host_protocol::WireEndpoint::File {
        source: openrows_host_protocol::WireFile::Disk {
            path: path.to_str().unwrap().into(),
            read_only: false,
            create: false,
        },
        attached: vec![],
    };
    let request = config.to_request().unwrap();
    let mut native = ConnectTrace::default();
    assert!(
        openrows_driver_sqlite::SqliteDriver
            .connect(&request, &mut native)
            .await
            .is_err()
    );
    let driver = HostedDriver::new(
        HostSupervisor::new(support::options(MemoryBudget::default())),
        Arc::new(openrows_driver_sqlite::SqliteDriver),
    );
    let mut hosted = ConnectTrace::default();
    assert!(matches!(
        driver.open(&request, &mut hosted).await,
        Err(DriverError::Server { .. })
    ));
    assert_eq!(hosted.failed, native.failed);
    assert_eq!(hosted.tls, native.tls);
    assert_eq!(
        hosted
            .completed
            .iter()
            .map(|(stage, _)| stage)
            .collect::<Vec<_>>(),
        native
            .completed
            .iter()
            .map(|(stage, _)| stage)
            .collect::<Vec<_>>()
    );
}
