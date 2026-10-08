// SPDX-License-Identifier: Apache-2.0
//! Drivers the app dispatches through (docs/05 §2): one per engine, naming
//! its SQL dialect and home database and opening [`Connection`]s.
use openrows_sql::Dialect;

use crate::connect::ConnectRequest;
use crate::connection::Connection;
use crate::schema::{SchemaChange, TableDetail};
use crate::structure::{AlterPlan, AlterTable};
use crate::trace::ConnectTrace;
use crate::{BoxFuture, DriverError};

/// Why requested SQL cannot be generated (an invalid name, type or change
/// set, or a feature the engine's generator lacks); shown to the user as is.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{0}")]
pub struct GenerateError(pub String);

/// A database driver for one engine.
pub trait Driver: Send + Sync {
    /// Product name for messages, e.g. `MySQL`.
    fn product(&self) -> &'static str;

    /// Lexical, classification and generation rules of the engine's SQL.
    fn dialect(&self) -> Dialect;

    /// Database opened when the profile names none (e.g. `postgres`).
    fn home_database(&self) -> &'static str;

    /// Open a connection, recording each stage in `trace`; the driver applies
    /// its own per-stage timeouts.
    ///
    /// # Errors
    /// `Network`, `Tls`, `Auth`, `Server`, `Protocol` or `Unsupported`.
    fn connect<'a>(
        &'a self,
        request: &'a ConnectRequest,
        trace: &'a mut ConnectTrace,
    ) -> BoxFuture<'a, Result<Box<dyn Connection>, DriverError>>;

    /// The table definition for the DDL view: the server's own definition
    /// when the catalog provides one (`MySQL`, `SQLite`), else rebuilt.
    ///
    /// # Errors
    /// Names the generator refuses.
    fn table_ddl(
        &self,
        (schema, table): (&str, &str),
        detail: &TableDetail,
    ) -> Result<String, GenerateError> {
        let definition = detail
            .info
            .as_ref()
            .and_then(|info| info.definition.clone());
        Ok(definition.unwrap_or_else(|| format!("-- {schema}.{table} was not found\n")))
    }

    /// True when [`Driver::alter_sql`] generates structure changes.
    fn supports_alter(&self) -> bool {
        false
    }

    /// Ordered DDL for a staged structure change set.
    ///
    /// # Errors
    /// Invalid or conflicting changes; by default the engine has no
    /// structure editor yet.
    fn alter_sql(
        &self,
        _target: &AlterTable<'_>,
        _changes: &[SchemaChange],
    ) -> Result<AlterPlan, GenerateError> {
        Err(GenerateError(format!(
            "Structure changes for {} tables are not available yet",
            self.product()
        )))
    }
}
