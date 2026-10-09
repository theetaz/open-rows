// SPDX-License-Identifier: Apache-2.0
//! The `PostgreSQL` [`Driver`] and the contract facets of [`PgSession`].
use openrows_driver_api::catalog::{
    ColumnSummary, DatabaseSummary, ObjectKind, ObjectSummary, SchemaSummary, TableStructure,
};
use openrows_driver_api::diagram::SchemaDiagram;
use openrows_driver_api::objects::{ObjectChange, ObjectPlan, RoutineKind};
use openrows_driver_api::schema::{SchemaChange, TableDetail};
use openrows_driver_api::structure::{AlterPlan, AlterTable};
use openrows_driver_api::{
    BoxFuture, Catalog, ConnectRequest, ConnectTrace, Connection, Driver, DriverError, Editor,
    Endpoint, GenerateError, RowEditInfo, RowSnapshot, ServerSummary,
};
use openrows_sql::Dialect;
use openrows_sql::select::BoundSql;

use crate::ddl::{alter_sql, create_table_sql, object_sql};
use crate::{BoundEdit, ConnectOptions, PgConfig, PgSession};

/// Maintenance database every `PostgreSQL` server creates; a profile without
/// a database connects here to list the others.
const HOME_DATABASE: &str = "postgres";

/// `PostgreSQL` and wire-compatible servers.
#[derive(Clone, Copy, Debug, Default)]
pub struct PgDriver;

impl PgDriver {
    /// Driver settings for `request`.
    ///
    /// # Errors
    /// `Unsupported` for a file endpoint.
    pub fn config(request: &ConnectRequest) -> Result<(PgConfig, ConnectOptions), DriverError> {
        let Endpoint::Server { host, port } = &request.endpoint else {
            return Err(DriverError::Unsupported(
                "PostgreSQL connects to a server, not a file".to_owned(),
            ));
        };
        let config = PgConfig {
            host: host.clone(),
            port: *port,
            user: request.user.clone(),
            password: request
                .password
                .as_ref()
                .map(|secret| secret.expose().to_owned()),
            database: request.database.clone(),
        };
        let options = ConnectOptions {
            application_name: request.application_name.clone(),
            ssl_mode: request.ssl_mode,
            tls: request.tls.clone(),
            via: request.via.clone(),
            timeouts: request.timeouts,
            startup_commands: request.startup_commands.clone(),
        };
        Ok((config, options))
    }
}

impl Driver for PgDriver {
    fn product(&self) -> &'static str {
        "PostgreSQL"
    }

    fn dialect(&self) -> Dialect {
        Dialect::Postgres
    }

    fn home_database(&self) -> &'static str {
        HOME_DATABASE
    }

    fn capabilities(&self) -> openrows_driver_api::Capabilities {
        crate::capabilities::CAPABILITIES
    }

    fn connect<'a>(
        &'a self,
        request: &'a ConnectRequest,
        trace: &'a mut ConnectTrace,
    ) -> BoxFuture<'a, Result<Box<dyn Connection>, DriverError>> {
        Box::pin(async move {
            let (config, options) = Self::config(request)?;
            let session = PgSession::connect_recording(&config, &options, trace).await?;
            Ok(Box::new(session) as Box<dyn Connection>)
        })
    }

    fn table_ddl(
        &self,
        (schema, table): (&str, &str),
        detail: &TableDetail,
    ) -> Result<String, GenerateError> {
        create_table_sql(schema, table, detail).map_err(|error| GenerateError(error.to_string()))
    }

    fn supports_alter(&self) -> bool {
        true
    }

    fn alter_sql(
        &self,
        target: &AlterTable<'_>,
        changes: &[SchemaChange],
    ) -> Result<AlterPlan, GenerateError> {
        alter_sql(target, changes).map_err(|error| GenerateError(error.to_string()))
    }

    fn object_sql(
        &self,
        change: &ObjectChange,
        source: Option<&TableDetail>,
    ) -> Result<ObjectPlan, GenerateError> {
        object_sql(change, source)
    }
}

impl Connection for PgSession {
    fn server(&self) -> &ServerSummary {
        self.summary()
    }

    fn admin(&mut self) -> Option<&mut dyn openrows_driver_api::Admin> {
        Some(self)
    }

    fn catalog(&mut self) -> Option<&mut dyn Catalog> {
        Some(self)
    }

    fn editor(&mut self) -> Option<&mut dyn Editor> {
        Some(self)
    }

    fn bulk_loader(&mut self) -> Option<&mut dyn openrows_driver_api::BulkLoader> {
        Some(self)
    }
}

impl Catalog for PgSession {
    fn list_databases(&mut self) -> BoxFuture<'_, Result<Vec<DatabaseSummary>, DriverError>> {
        Box::pin(PgSession::list_databases(self))
    }

    fn list_schemas(&mut self) -> BoxFuture<'_, Result<Vec<SchemaSummary>, DriverError>> {
        Box::pin(PgSession::list_schemas(self))
    }

    fn list_objects<'a>(
        &'a mut self,
        schema: &'a str,
        kinds: &'a [ObjectKind],
    ) -> BoxFuture<'a, Result<Vec<ObjectSummary>, DriverError>> {
        Box::pin(PgSession::list_objects(self, schema, kinds))
    }

    fn search_index(&mut self) -> BoxFuture<'_, Result<Vec<ObjectSummary>, DriverError>> {
        Box::pin(PgSession::search_index(self))
    }

    fn table_columns<'a>(
        &'a mut self,
        schema: &'a str,
        table: &'a str,
    ) -> BoxFuture<'a, Result<Vec<ColumnSummary>, DriverError>> {
        Box::pin(PgSession::table_columns(self, schema, table))
    }

    fn table_structure<'a>(
        &'a mut self,
        schema: &'a str,
        table: &'a str,
    ) -> BoxFuture<'a, Result<TableStructure, DriverError>> {
        Box::pin(PgSession::table_structure(self, schema, table))
    }

    fn table_detail<'a>(
        &'a mut self,
        schema: &'a str,
        table: &'a str,
    ) -> BoxFuture<'a, Result<TableDetail, DriverError>> {
        Box::pin(PgSession::table_detail(self, schema, table))
    }

    fn schema_fingerprint(&mut self) -> BoxFuture<'_, Result<Option<String>, DriverError>> {
        Box::pin(PgSession::schema_fingerprint(self))
    }

    fn schema_diagram<'a>(
        &'a mut self,
        schema: &'a str,
        max_tables: usize,
    ) -> BoxFuture<'a, Result<SchemaDiagram, DriverError>> {
        Box::pin(PgSession::schema_diagram(self, schema, max_tables))
    }

    fn routine_definition<'a>(
        &'a mut self,
        routine: (&'a str, &'a str),
        kind: RoutineKind,
        arguments: Option<&'a str>,
    ) -> BoxFuture<'a, Result<Option<String>, DriverError>> {
        Box::pin(PgSession::routine_definition(
            self, routine, kind, arguments,
        ))
    }
}

impl Editor for PgSession {
    fn row_edit_info<'a>(
        &'a mut self,
        schema: &'a str,
        table: &'a str,
    ) -> BoxFuture<'a, Result<RowEditInfo, DriverError>> {
        Box::pin(PgSession::row_edit_info(self, schema, table))
    }

    fn edit_snapshot<'a>(
        &'a mut self,
        target: (&'a str, &'a str),
        keys: &'a [(String, String)],
    ) -> BoxFuture<'a, Result<RowSnapshot, DriverError>> {
        Box::pin(PgSession::edit_snapshot(self, target, keys))
    }

    fn commit_edits<'a>(
        &'a mut self,
        edits: &'a [BoundSql],
    ) -> BoxFuture<'a, Result<(), DriverError>> {
        Box::pin(async move {
            let edits: Vec<BoundEdit> = edits
                .iter()
                .map(|edit| BoundEdit {
                    sql: edit.sql.clone(),
                    params: edit.params.clone(),
                })
                .collect();
            PgSession::commit_edits(self, &edits).await
        })
    }

    fn commit_ddl<'a>(
        &'a mut self,
        statements: &'a [String],
    ) -> BoxFuture<'a, Result<(), DriverError>> {
        Box::pin(PgSession::commit_ddl(self, statements))
    }
}
