// SPDX-License-Identifier: Apache-2.0
//! A wire-compatible server lacks features of the engine it imitates and says
//! so on connect (`Connection::capabilities`). [`refine`] lets every group
//! gate on what the server really offers, so a gap is a skip, not a failure.

use openrows_driver_api::objects::{ObjectChange, ObjectPlan};
use openrows_driver_api::schema::{SchemaChange, TableDetail};
use openrows_driver_api::structure::{AlterPlan, AlterTable};
use openrows_driver_api::{
    BoxFuture, Capabilities, ConnectRequest, ConnectTrace, Connection, Driver, DriverError,
    Feature, GenerateError,
};
use openrows_sql::Dialect;

use crate::{Failure, Target};

/// A driver as the connected server narrows it.
struct Narrowed {
    driver: Box<dyn Driver>,
    capabilities: Capabilities,
}

impl Driver for Narrowed {
    fn product(&self) -> &'static str {
        self.driver.product()
    }

    fn dialect(&self) -> Dialect {
        self.driver.dialect()
    }

    fn home_database(&self) -> &'static str {
        self.driver.home_database()
    }

    fn capabilities(&self) -> Capabilities {
        self.capabilities
    }

    fn connect<'a>(
        &'a self,
        request: &'a ConnectRequest,
        trace: &'a mut ConnectTrace,
    ) -> BoxFuture<'a, Result<Box<dyn Connection>, DriverError>> {
        self.driver.connect(request, trace)
    }

    fn table_ddl(
        &self,
        table: (&str, &str),
        detail: &TableDetail,
    ) -> Result<String, GenerateError> {
        self.driver.table_ddl(table, detail)
    }

    fn supports_alter(&self) -> bool {
        self.driver.supports_alter() && self.capabilities.supports(Feature::StructureChanges)
    }

    fn alter_sql(
        &self,
        target: &AlterTable<'_>,
        changes: &[SchemaChange],
    ) -> Result<AlterPlan, GenerateError> {
        self.driver.alter_sql(target, changes)
    }

    fn object_sql(
        &self,
        change: &ObjectChange,
        source: Option<&TableDetail>,
    ) -> Result<ObjectPlan, GenerateError> {
        if !self.capabilities.supports(Feature::TableLifecycle) {
            return Err(GenerateError(format!(
                "{} objects cannot be changed on this server",
                self.driver.product()
            )));
        }
        self.driver.object_sql(change, source)
    }
}

/// `target` with the capabilities its server reports; only `PostgreSQL` and
/// `MySQL` have wire-compatible products, so other engines skip the connection.
pub(crate) async fn refine(target: Target) -> Result<Target, Failure> {
    if !matches!(target.driver.dialect(), Dialect::Postgres | Dialect::MySql) {
        return Ok(target);
    }
    let connection = target.connect().await?;
    let server = connection.capabilities();
    connection.close().await?;
    let Some(capabilities) = server else {
        return Ok(target);
    };
    Ok(Target {
        driver: Box::new(Narrowed {
            driver: target.driver,
            capabilities,
        }),
        ..target
    })
}
