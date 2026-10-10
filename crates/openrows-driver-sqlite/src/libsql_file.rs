// SPDX-License-Identifier: Apache-2.0
//! SQLite-compatible libSQL files reuse one bundled `SQLite` implementation.
use crate::SqliteDriver;
use openrows_driver_api::{
    BoxFuture, ConnectRequest, ConnectTrace, Connection, Driver, DriverError, GenerateError,
};
use openrows_driver_api::{
    objects::{ObjectChange, ObjectPlan},
    schema::{SchemaChange, TableDetail},
    structure::{AlterPlan, AlterTable},
};
use openrows_sql::Dialect;

/// Local libSQL files restricted to SQLite-compatible features.
#[derive(Clone, Copy, Debug, Default)]
pub struct LibsqlFileDriver;

impl Driver for LibsqlFileDriver {
    fn product(&self) -> &'static str {
        "libSQL file"
    }
    fn dialect(&self) -> Dialect {
        Dialect::Sqlite
    }
    fn home_database(&self) -> &'static str {
        "main"
    }
    fn capabilities(&self) -> openrows_driver_api::Capabilities {
        SqliteDriver.capabilities()
    }
    fn supports_alter(&self) -> bool {
        SqliteDriver.supports_alter()
    }
    fn alter_sql(
        &self,
        target: &AlterTable<'_>,
        changes: &[SchemaChange],
    ) -> Result<AlterPlan, GenerateError> {
        SqliteDriver.alter_sql(target, changes)
    }
    fn object_sql(
        &self,
        change: &ObjectChange,
        source: Option<&TableDetail>,
    ) -> Result<ObjectPlan, GenerateError> {
        SqliteDriver.object_sql(change, source)
    }
    fn connect<'a>(
        &'a self,
        request: &'a ConnectRequest,
        trace: &'a mut ConnectTrace,
    ) -> BoxFuture<'a, Result<Box<dyn Connection>, DriverError>> {
        crate::connect::connect_file(request, trace, true)
    }
}
