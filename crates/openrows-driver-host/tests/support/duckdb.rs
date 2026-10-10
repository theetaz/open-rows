// SPDX-License-Identifier: Apache-2.0
//! Native `DuckDB` fixture paths and sinks; no native library in the parent.
use openrows_driver_api::{ConnectRequest, ConnectTrace, DriverError};
use openrows_host_client::{
    DuckdbMetadata, HostSupervisor, HostedConnection, HostedDriver, MemoryBudget,
};
use std::sync::Arc;

#[path = "duckdb_launch.rs"]
mod launch;
pub use launch::options;
/// Pure parent metadata plus a lazily launched native process.
pub fn driver() -> HostedDriver {
    HostedDriver::new(
        HostSupervisor::new(options(MemoryBudget::default())),
        Arc::new(DuckdbMetadata),
    )
}
/// Synthetic local connect request.
pub fn request(source: openrows_driver_api::FileSource) -> ConnectRequest {
    ConnectRequest {
        endpoint: openrows_driver_api::Endpoint::File {
            source,
            attached: vec![],
        },
        database: "main".into(),
        user: String::new(),
        password: None,
        application_name: "test-only".into(),
        ssl_mode: openrows_driver_api::SslMode::Disable,
        tls: openrows_driver_api::TlsFiles::default(),
        via: None,
        timeouts: openrows_driver_api::ConnectTimeouts::default(),
        driver_options: std::collections::BTreeMap::default(),
        startup_commands: vec![],
    }
}
/// Open one isolated native fixture connection.
pub async fn open(
    source: openrows_driver_api::FileSource,
) -> Result<HostedConnection, DriverError> {
    #[cfg(target_os = "macos")]
    super::image::prepare().await.map_err(DriverError::Host)?;
    driver()
        .open(&request(source), &mut ConnectTrace::default())
        .await
}
/// Named profile-local in-memory fixture identity.
pub fn memory(name: &str) -> openrows_driver_api::FileSource {
    openrows_driver_api::FileSource::Memory { name: name.into() }
}
