// SPDX-License-Identifier: Apache-2.0
//! Native-free metadata and SQL admission for the production hosted engine.
use openrows_driver_api::{
    BoxFuture, Capabilities, ConnectRequest, ConnectTrace, Connection, Driver, DriverError,
    Feature, Location, catalog::ObjectKind,
};

/// Tier 2 `DuckDB` contract: query, catalog, guarded edits and transactional
/// DDL, all on the one shared hosted session (docs/05 §20).
pub const DUCKDB_CAPABILITIES: Capabilities = Capabilities {
    location: Location::File,
    features: &[
        Feature::Cancel,
        Feature::BindParameters,
        Feature::Schemas,
        Feature::Catalog,
        Feature::RowEdits,
        Feature::TransactionalDdl,
        Feature::StartupCommands,
        Feature::SchemaFingerprint,
    ],
    object_kinds: &[
        ObjectKind::Table,
        ObjectKind::View,
        ObjectKind::Function,
        ObjectKind::Sequence,
    ],
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
