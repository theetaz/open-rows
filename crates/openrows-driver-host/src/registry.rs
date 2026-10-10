// SPDX-License-Identifier: Apache-2.0
//! Entry-specific driver registration; no dynamic loading or in-process fallback.
#[cfg(feature = "test-sqlite")]
use openrows_driver_api::Driver;
use openrows_driver_api::{BoxFuture, ConnectRequest, ConnectTrace, Connection, DriverError};
use openrows_host_protocol::HostDriverId;

pub(crate) type Connect = for<'a> fn(
    HostDriverId,
    &'a ConnectRequest,
    &'a mut ConnectTrace,
) -> BoxFuture<'a, Result<Box<dyn Connection>, DriverError>>;

pub(crate) struct Registry {
    pub(crate) driver: HostDriverId,
    pub(crate) connect: Connect,
}

pub(crate) fn production<'a>(
    driver: HostDriverId,
    request: &'a ConnectRequest,
    trace: &'a mut ConnectTrace,
) -> BoxFuture<'a, Result<Box<dyn Connection>, DriverError>> {
    Box::pin(async move {
        match driver {
            HostDriverId::Probe => Err(probe_error()),
            HostDriverId::Duckdb => {
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
            HostDriverId::SqliteTest => Err(DriverError::Unsupported(
                "test SQLite is not compiled into this helper".into(),
            )),
        }
    })
}

// Keep this entry independent of production so the linker can omit all native DuckDB code.
#[cfg(feature = "test-sqlite")]
pub(crate) fn sqlite<'a>(
    driver: HostDriverId,
    request: &'a ConnectRequest,
    trace: &'a mut ConnectTrace,
) -> BoxFuture<'a, Result<Box<dyn Connection>, DriverError>> {
    Box::pin(async move {
        match driver {
            HostDriverId::Probe => Err(probe_error()),
            HostDriverId::Duckdb => Err(DriverError::Unsupported(
                "DuckDB is not compiled into this fixture".into(),
            )),
            HostDriverId::SqliteTest => tokio::time::timeout(
                request.timeouts.tcp.min(std::time::Duration::from_secs(10)),
                openrows_driver_sqlite::SqliteDriver.connect(request, trace),
            )
            .await
            .map_err(|_| DriverError::Server {
                code: "57014".into(),
                message: "file open deadline exceeded".into(),
            })?,
        }
    })
}

fn probe_error() -> DriverError {
    DriverError::Unsupported("handshake probe cannot open a database".into())
}
