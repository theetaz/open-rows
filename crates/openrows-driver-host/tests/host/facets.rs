// SPDX-License-Identifier: Apache-2.0
//! Multi-frame catalog and lost commit acknowledgement acceptance tests.
use crate::{adapter, support};
use openrows_driver_api::{ConnectTrace, DriverError, Session, catalog::ObjectKind};
use openrows_host_client::{HostSupervisor, HostedDriver, MemoryBudget};
use openrows_host_protocol::{WireEndpoint, WireFile};
use std::sync::Arc;
#[tokio::test]
async fn catalog_larger_than_control_frame_is_acknowledged_in_bounded_chunks() {
    let _permit = support::host_test().await.unwrap();
    let mut connection = adapter::connection().await;
    let suffix = "x".repeat(220);
    for group in 0..10 {
        let ddl: Vec<_> = (group * 500..(group + 1) * 500)
            .map(|n| format!("CREATE TABLE t{n}_{suffix}(id integer)"))
            .collect();
        connection.editor().unwrap().commit_ddl(&ddl).await.unwrap();
    }
    let objects = connection
        .catalog()
        .unwrap()
        .list_objects("main", &[ObjectKind::Table])
        .await
        .unwrap();
    assert_eq!(objects.len(), 5000);
    assert!(
        objects
            .iter()
            .map(|object| object.name.len())
            .sum::<usize>()
            > 1024 * 1024
    );
    connection.ping().await.unwrap();
    connection.close().await.unwrap();
}
#[tokio::test]
async fn committed_write_without_ack_is_uncertain_and_never_replayed() {
    let _permit = support::host_test().await.unwrap();
    let directory = tempfile::tempdir_in(".").unwrap();
    let path = std::fs::canonicalize(directory.path())
        .unwrap()
        .join("test-only.sqlite");
    let mut config = support::config();
    config.endpoint = WireEndpoint::File {
        source: WireFile::Disk {
            path: path.to_str().unwrap().into(),
            read_only: false,
            create: true,
        },
        attached: vec![],
    };
    let driver = HostedDriver::new(
        HostSupervisor::new(support::options(MemoryBudget::default())),
        Arc::new(openrows_driver_sqlite::SqliteDriver),
    );
    let request = config.to_request().unwrap();
    let mut connection = driver
        .open(&request, &mut ConnectTrace::default())
        .await
        .unwrap();
    let failure = connection.host_failure().unwrap();
    let error = openrows_driver_api::Editor::commit_ddl(
        &mut connection,
        &["OPENROWS_TEST_COMMIT_ABORT".into()],
    )
    .await
    .unwrap_err();
    assert!(matches!(error, DriverError::Server { ref code, .. } if code == "08007"));
    assert!(!error.is_connection_lost());
    assert!(matches!(failure.await, DriverError::Host(_)));
    drop(connection);
    let mut fresh = driver
        .open(&request, &mut ConnectTrace::default())
        .await
        .unwrap();
    let tables =
        openrows_driver_api::Catalog::list_objects(&mut fresh, "main", &[ObjectKind::Table])
            .await
            .unwrap();
    assert_eq!(
        tables
            .iter()
            .filter(|table| table.name == "committed_once")
            .count(),
        1
    );
    Box::new(fresh).close().await.unwrap();
}
