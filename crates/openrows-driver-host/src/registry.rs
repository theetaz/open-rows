// SPDX-License-Identifier: Apache-2.0
//! Compile-time driver selection; no dynamic loading or in-process fallback.
#[cfg(feature = "test-sqlite")]
use openrows_driver_api::Driver;
use openrows_driver_api::{ConnectRequest, ConnectTrace, Connection, DriverError};
/// Local test registry, extended with a host-only `DuckDB` crate at M2-04.
pub(crate) async fn connect(
    request: &ConnectRequest,
    trace: &mut ConnectTrace,
) -> Result<Box<dyn Connection>, DriverError> {
    #[cfg(not(feature = "test-sqlite"))]
    {
        let _ = (request, trace);
        Err(DriverError::Unsupported(
            "no production hosted driver is registered".into(),
        ))
    }
    #[cfg(feature = "test-sqlite")]
    {
        let deadline = request.timeouts.tcp.min(std::time::Duration::from_secs(10));
        tokio::time::timeout(
            deadline,
            openrows_driver_sqlite::SqliteDriver.connect(request, trace),
        )
        .await
        .map_err(|_| DriverError::Server {
            code: "57014".into(),
            message: "file open deadline exceeded".into(),
        })?
    }
}
