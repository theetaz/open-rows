// SPDX-License-Identifier: Apache-2.0
//! Native-free metadata and SQL admission for the production hosted engine.
use openrows_driver_api::{
    BoxFuture, Capabilities, ConnectRequest, ConnectTrace, Connection, Driver, DriverError,
    Feature, Location,
};

/// Query-only `DuckDB` contract; catalog/edit/background work follows in M2-04b.
pub const DUCKDB_CAPABILITIES: Capabilities = Capabilities {
    location: Location::File,
    features: &[
        Feature::Cancel,
        Feature::BindParameters,
        Feature::StartupCommands,
    ],
    object_kinds: &[],
};

/// Cheap parent-side engine metadata; it can never open a native database.
#[derive(Clone, Copy, Debug, Default)]
pub struct DuckdbMetadata;
impl Driver for DuckdbMetadata {
    fn product(&self) -> &'static str {
        "DuckDB"
    }
    fn dialect(&self) -> openrows_sql::Dialect {
        openrows_sql::Dialect::Duckdb
    }
    fn home_database(&self) -> &'static str {
        "main"
    }
    fn capabilities(&self) -> Capabilities {
        DUCKDB_CAPABILITIES
    }
    fn connect<'a>(
        &'a self,
        _request: &'a ConnectRequest,
        _trace: &'a mut ConnectTrace,
    ) -> BoxFuture<'a, Result<Box<dyn Connection>, DriverError>> {
        Box::pin(async {
            Err(DriverError::Unsupported(
                "DuckDB requires the driver host".into(),
            ))
        })
    }
}

pub(crate) fn admit(sql: &str) -> Result<(), DriverError> {
    if openrows_sql::duckdb::has_attachment(sql) {
        return Err(DriverError::Unsupported(
            "DuckDB ATTACH is unavailable until attached-file reservations are supported".into(),
        ));
    }
    Ok(())
}
