// SPDX-License-Identifier: Apache-2.0
//! Catalog and edit facets of the shared `DuckDB` session, queued on its own
//! actor: a writable file admits no second metadata or edit connection.
use openrows_core::FacetPort;
use openrows_driver_api::catalog::{
    ColumnSummary, DatabaseSummary, ObjectKind, ObjectSummary, SchemaSummary, TableStructure,
};
use openrows_driver_api::inspection::{ObjectDetail, ObjectIdentity};
use openrows_driver_api::schema::TableDetail;
use openrows_driver_api::{
    BoxFuture, Catalog, Connection, DriverError, Editor, ExecSummary, ResultSink, RowEditInfo,
    RowSnapshot, ServerSummary, Session, catalog, editor,
};
use openrows_sql::select::BoundSql;

/// A borrowed view of the owning session; closing it leaves that session open.
pub(crate) struct SharedFacets {
    port: FacetPort,
    server: ServerSummary,
}

impl SharedFacets {
    /// Borrow the session behind `port`, reading its server summary in queue order.
    pub(crate) async fn new(port: FacetPort) -> Result<Self, DriverError> {
        let server = port
            .call(|connection| {
                let server = connection.server().clone();
                Box::pin(async move { Ok(server) })
            })
            .await?;
        Ok(Self { port, server })
    }

    fn on_catalog<T: Send + 'static>(
        &self,
        call: impl for<'s> FnOnce(&'s mut dyn Catalog) -> BoxFuture<'s, Result<T, DriverError>>
        + Send
        + 'static,
    ) -> BoxFuture<'static, Result<T, DriverError>> {
        let port = self.port.clone();
        Box::pin(async move {
            port.call(move |connection| match catalog(connection) {
                Ok(facet) => call(facet),
                Err(error) => Box::pin(async move { Err(error) }),
            })
            .await
        })
    }

    fn on_editor<T: Send + 'static>(
        &self,
        call: impl for<'s> FnOnce(&'s mut dyn Editor) -> BoxFuture<'s, Result<T, DriverError>>
        + Send
        + 'static,
    ) -> BoxFuture<'static, Result<T, DriverError>> {
        let port = self.port.clone();
        Box::pin(async move {
            port.call(move |connection| match editor(connection) {
                Ok(facet) => call(facet),
                Err(error) => Box::pin(async move { Err(error) }),
            })
            .await
        })
    }
}

impl Session for SharedFacets {
    fn execute<'a>(
        &'a mut self,
        _sql: &'a str,
        _sink: &'a mut dyn ResultSink,
    ) -> BoxFuture<'a, Result<ExecSummary, DriverError>> {
        Box::pin(async {
            Err(DriverError::Unsupported(
                "run SQL in the owning DuckDB session".into(),
            ))
        })
    }
    fn close(self: Box<Self>) -> BoxFuture<'static, Result<(), DriverError>> {
        Box::pin(async { Ok(()) })
    }
}

impl Connection for SharedFacets {
    fn server(&self) -> &ServerSummary {
        &self.server
    }
    fn catalog(&mut self) -> Option<&mut dyn Catalog> {
        Some(self)
    }
    fn editor(&mut self) -> Option<&mut dyn Editor> {
        Some(self)
    }
}

impl Catalog for SharedFacets {
    fn list_databases(&mut self) -> BoxFuture<'_, Result<Vec<DatabaseSummary>, DriverError>> {
        self.on_catalog(|facet| facet.list_databases())
    }
    fn list_schemas(&mut self) -> BoxFuture<'_, Result<Vec<SchemaSummary>, DriverError>> {
        self.on_catalog(|facet| facet.list_schemas())
    }
    fn list_objects<'a>(
        &'a mut self,
        schema: &'a str,
        kinds: &'a [ObjectKind],
    ) -> BoxFuture<'a, Result<Vec<ObjectSummary>, DriverError>> {
        let (schema, kinds) = (schema.to_owned(), kinds.to_vec());
        self.on_catalog(move |facet| {
            Box::pin(async move { facet.list_objects(&schema, &kinds).await })
        })
    }
    fn search_index(&mut self) -> BoxFuture<'_, Result<Vec<ObjectSummary>, DriverError>> {
        self.on_catalog(|facet| facet.search_index())
    }
    fn table_columns<'a>(
        &'a mut self,
        schema: &'a str,
        table: &'a str,
    ) -> BoxFuture<'a, Result<Vec<ColumnSummary>, DriverError>> {
        let (schema, table) = (schema.to_owned(), table.to_owned());
        self.on_catalog(move |facet| {
            Box::pin(async move { facet.table_columns(&schema, &table).await })
        })
    }
    fn table_structure<'a>(
        &'a mut self,
        schema: &'a str,
        table: &'a str,
    ) -> BoxFuture<'a, Result<TableStructure, DriverError>> {
        let (schema, table) = (schema.to_owned(), table.to_owned());
        self.on_catalog(move |facet| {
            Box::pin(async move { facet.table_structure(&schema, &table).await })
        })
    }
    fn table_detail<'a>(
        &'a mut self,
        schema: &'a str,
        table: &'a str,
    ) -> BoxFuture<'a, Result<TableDetail, DriverError>> {
        let (schema, table) = (schema.to_owned(), table.to_owned());
        self.on_catalog(move |facet| {
            Box::pin(async move { facet.table_detail(&schema, &table).await })
        })
    }
    fn schema_fingerprint(&mut self) -> BoxFuture<'_, Result<Option<String>, DriverError>> {
        self.on_catalog(|facet| facet.schema_fingerprint())
    }
    fn object_detail<'a>(
        &'a mut self,
        object: &'a ObjectIdentity,
    ) -> BoxFuture<'a, Result<ObjectDetail, DriverError>> {
        let object = object.clone();
        self.on_catalog(move |facet| Box::pin(async move { facet.object_detail(&object).await }))
    }
}

impl Editor for SharedFacets {
    fn row_edit_info<'a>(
        &'a mut self,
        schema: &'a str,
        table: &'a str,
    ) -> BoxFuture<'a, Result<RowEditInfo, DriverError>> {
        let (schema, table) = (schema.to_owned(), table.to_owned());
        self.on_editor(move |facet| {
            Box::pin(async move { facet.row_edit_info(&schema, &table).await })
        })
    }
    fn edit_snapshot<'a>(
        &'a mut self,
        (schema, table): (&'a str, &'a str),
        keys: &'a [(String, String)],
    ) -> BoxFuture<'a, Result<RowSnapshot, DriverError>> {
        let (schema, table, keys) = (schema.to_owned(), table.to_owned(), keys.to_vec());
        self.on_editor(move |facet| {
            Box::pin(async move { facet.edit_snapshot((&schema, &table), &keys).await })
        })
    }
    fn commit_edits<'a>(
        &'a mut self,
        edits: &'a [BoundSql],
    ) -> BoxFuture<'a, Result<(), DriverError>> {
        let edits = edits.to_vec();
        self.on_editor(move |facet| Box::pin(async move { facet.commit_edits(&edits).await }))
    }
    fn commit_ddl<'a>(
        &'a mut self,
        statements: &'a [String],
    ) -> BoxFuture<'a, Result<(), DriverError>> {
        let statements = statements.to_vec();
        self.on_editor(move |facet| Box::pin(async move { facet.commit_ddl(&statements).await }))
    }
}
