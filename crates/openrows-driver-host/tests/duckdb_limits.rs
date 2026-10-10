// SPDX-License-Identifier: Apache-2.0
//! Exclusive large rows and startup statement draining through the hosted adapter.
#[path = "support/duckdb.rs"]
mod fixture;
#[cfg(target_os = "macos")]
#[path = "support/duckdb_image.rs"]
mod image;
use arrow_array::{RecordBatch, StringArray};
use arrow_schema::SchemaRef;
use fixture::*;
use openrows_driver_api::{ConnectTrace, DriverError, ResultSink, Session, SinkFlow};
use openrows_host_client::{HostFailure, HostState, HostSupervisor, MemoryBudget};
#[derive(Default)]
struct Size(usize);
impl ResultSink for Size {
    fn begin_result(&mut self, _: SchemaRef) {}
    fn push_batch(&mut self, batch: RecordBatch) -> SinkFlow {
        if let Some(array) = batch.column(0).as_any().downcast_ref::<StringArray>() {
            self.0 = array.value(0).len();
        }
        SinkFlow::Continue
    }
}
#[tokio::test]
async fn latched_failure_close_waits_for_reap() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    image::prepare().await?;
    let host = HostSupervisor::new(options(MemoryBudget::default()))
        .spawn()
        .await?;
    let state = host.subscribe();
    let endpoints = host.directory().to_path_buf();
    host.poison(HostFailure::MemoryLimit);
    assert_eq!(host.close().await, Err(HostFailure::MemoryLimit));
    assert_eq!(*state.borrow(), HostState::Failed(HostFailure::MemoryLimit));
    assert!(!endpoints.exists());
    Ok(())
}
#[tokio::test]
async fn exclusive_large_row_and_native_ceiling_are_typed() -> Result<(), Box<dyn std::error::Error>>
{
    let mut connection = open(memory("large-row-test")).await?;
    let mut sink = Size::default();
    connection
        .execute("SELECT repeat('x', 8388608)", &mut sink)
        .await?;
    assert_eq!(sink.0, 8_388_608);
    assert!(connection.metrics().encoded_bytes > 4 * 1024 * 1024);
    assert!(connection.metrics().decoded_bytes <= 64 * 1024 * 1024);
    let result = connection
        .execute("SELECT repeat('x', 67108865)", &mut sink)
        .await;
    match result {
        Err(DriverError::Unsupported(message)) => {
            assert!(message.contains("64 MiB"));
            connection.ping().await?;
        }
        Err(DriverError::Server { message, .. }) => {
            assert!(message.to_lowercase().contains("memory"));
            connection.ping().await?;
        }
        Err(DriverError::Host(HostFailure::MemoryLimit)) => {
            assert!(matches!(connection.ping().await, Err(DriverError::Host(_))));
            let _ = Box::new(connection).close().await;
            let mut fresh = open(memory("large-row-test")).await?;
            fresh.ping().await?;
            Box::new(fresh).close().await?;
            return Ok(());
        }
        other => return Err(format!("oversized result: {other:?}").into()),
    }
    Box::new(connection).close().await?;
    Ok(())
}
#[tokio::test]
async fn startup_drains_select_before_later_statements() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    image::prepare().await?;
    let mut config = request(memory("startup-test"));
    config
        .startup_commands
        .push("SELECT 1; CREATE TABLE startup_data AS SELECT 7 AS value".into());
    let mut connection = driver().open(&config, &mut ConnectTrace::default()).await?;
    assert_eq!(
        connection
            .execute(
                "SELECT * FROM startup_data",
                &mut openrows_driver_api::DiscardSink
            )
            .await?
            .rows,
        1
    );
    Box::new(connection).close().await?;
    Ok(())
}
