// SPDX-License-Identifier: Apache-2.0
//! Compile-time driver selection; no dynamic loading or in-process fallback.
#[cfg(feature = "test-sqlite")]
use openrows_driver_api::Driver;
use openrows_driver_api::{ConnectRequest, ConnectTrace, Connection, DriverError};
/// Local test registry, extended with a host-only `DuckDB` crate at M2-04.
pub(crate) async fn connect(
    driver: openrows_host_protocol::HostDriverId,
    request: &ConnectRequest,
    trace: &mut ConnectTrace,
) -> Result<Box<dyn Connection>, DriverError> {
    match driver {
        openrows_host_protocol::HostDriverId::Probe => Err(DriverError::Unsupported(
            "handshake probe cannot open a database".into(),
        )),
        openrows_host_protocol::HostDriverId::Duckdb => {
            #[cfg(feature = "duckdb")]
            {
                crate::duckdb::connect(request, trace).await
            }
            #[cfg(not(feature = "duckdb"))]
            {
                let _ = (request, trace);
                Err(DriverError::Unsupported(
                    "DuckDB is not compiled into this helper".into(),
                ))
            }
        }
        openrows_host_protocol::HostDriverId::SqliteTest => {
            #[cfg(feature = "test-sqlite")]
            {
                tokio::time::timeout(
                    request.timeouts.tcp.min(std::time::Duration::from_secs(10)),
                    openrows_driver_sqlite::SqliteDriver.connect(request, trace),
                )
                .await
                .map_err(|_| DriverError::Server {
                    code: "57014".into(),
                    message: "file open deadline exceeded".into(),
                })?
            }
            #[cfg(not(feature = "test-sqlite"))]
            {
                let _ = (request, trace);
                Err(DriverError::Unsupported(
                    "test SQLite is not compiled into this helper".into(),
                ))
            }
        }
    }
}
