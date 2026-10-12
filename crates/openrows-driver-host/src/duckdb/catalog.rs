// SPDX-License-Identifier: Apache-2.0
//! Catalog facet read from `duckdb_*` functions on the one shared native session.
use super::catalog_sql as sql;
use super::session::DuckdbSession;
use super::text::{self, Row};
use openrows_driver_api::catalog::{
    ColumnSummary, DatabaseSummary, ObjectKind, ObjectSummary, SchemaSummary, TableStructure,
};
use openrows_driver_api::inspection::{ObjectDetail, ObjectIdentity};
use openrows_driver_api::schema::TableDetail;
use openrows_driver_api::{BoxFuture, Catalog, DriverError};

/// Kinds the tree lists, in tree order.
const KINDS: [ObjectKind; 4] = [
    ObjectKind::Table,
    ObjectKind::View,
    ObjectKind::Function,
    ObjectKind::Sequence,
];

impl DuckdbSession {
    pub(super) fn query(
        &self,
        sql: &str,
        params: &[Option<&str>],
        width: usize,
    ) -> Result<Vec<Row>, DriverError> {
        text::rows(&self.native, sql, params, width)
    }

    fn databases(&self) -> Result<Vec<DatabaseSummary>, DriverError> {
        self.query(sql::DATABASES, &[], 3)?
            .iter()
            .map(|row| {
                let name = if text::flag(row, 1) {
                    "main".to_owned()
                } else {
                    text::value(row, 0)?
                };
                Ok(DatabaseSummary {
                    name,
                    system: false,
                    comment: text::flag(row, 2).then(|| "read-only".to_owned()),
                })
            })
            .collect()
    }

    fn schemas(&self) -> Result<Vec<SchemaSummary>, DriverError> {
        self.query(sql::SCHEMAS, &[], 3)?
            .iter()
            .map(|row| {
                Ok(SchemaSummary {
                    name: text::value(row, 0)?,
                    system: text::flag(row, 1),
                    comment: text::optional(row, 2),
                })
            })
            .collect()
    }

    fn objects(
        &self,
        schema: Option<&str>,
        kinds: &[ObjectKind],
    ) -> Result<Vec<ObjectSummary>, DriverError> {
        let mut objects = Vec::new();
        for row in self.query(sql::OBJECTS, &[schema], 6)? {
            let kind = ObjectKind::parse(&text::value(&row, 0)?)
                .ok_or_else(|| DriverError::Protocol("unknown DuckDB object kind".into()))?;
            if !kinds.contains(&kind) {
                continue;
            }
            objects.push(ObjectSummary {
                schema: text::value(&row, 1)?,
                name: text::value(&row, 2)?,
                kind,
                arguments: text::optional(&row, 3),
                row_estimate: text::optional(&row, 4).and_then(|value| value.parse().ok()),
                comment: text::optional(&row, 5),
            });
        }
        Ok(objects)
    }

    pub(super) fn columns(
        &self,
        schema: &str,
        table: &str,
    ) -> Result<Vec<ColumnSummary>, DriverError> {
        self.query(sql::COLUMNS, &[Some(schema), Some(table)], 6)?
            .iter()
            .map(|row| {
                let primary_key = text::flag(row, 5);
                Ok(ColumnSummary {
                    name: text::value(row, 0)?,
                    data_type: text::value(row, 1)?,
                    nullable: text::flag(row, 2) && !primary_key,
                    default: text::optional(row, 3),
                    comment: text::optional(row, 4),
                    primary_key,
                    enum_labels: None,
                })
            })
            .collect()
    }

    fn fingerprint(&self) -> Result<Option<String>, DriverError> {
        Ok(self
            .query(sql::FINGERPRINT, &[], 1)?
            .first()
            .and_then(|row| text::optional(row, 0)))
    }
}

impl Catalog for DuckdbSession {
    fn list_databases(&mut self) -> BoxFuture<'_, Result<Vec<DatabaseSummary>, DriverError>> {
        Box::pin(async move { self.databases() })
    }
    fn list_schemas(&mut self) -> BoxFuture<'_, Result<Vec<SchemaSummary>, DriverError>> {
        Box::pin(async move { self.schemas() })
    }
    fn list_objects<'a>(
        &'a mut self,
        schema: &'a str,
        kinds: &'a [ObjectKind],
    ) -> BoxFuture<'a, Result<Vec<ObjectSummary>, DriverError>> {
        Box::pin(async move { self.objects(Some(schema), kinds) })
    }
    fn search_index(&mut self) -> BoxFuture<'_, Result<Vec<ObjectSummary>, DriverError>> {
        Box::pin(async move { self.objects(None, &KINDS) })
    }
    fn table_columns<'a>(
        &'a mut self,
        schema: &'a str,
        table: &'a str,
    ) -> BoxFuture<'a, Result<Vec<ColumnSummary>, DriverError>> {
        Box::pin(async move { self.columns(schema, table) })
    }
    fn table_structure<'a>(
        &'a mut self,
        schema: &'a str,
        table: &'a str,
    ) -> BoxFuture<'a, Result<TableStructure, DriverError>> {
        Box::pin(async move { self.structure(schema, table) })
    }
    fn table_detail<'a>(
        &'a mut self,
        schema: &'a str,
        table: &'a str,
    ) -> BoxFuture<'a, Result<TableDetail, DriverError>> {
        Box::pin(async move { self.detail(schema, table) })
    }
    fn schema_fingerprint(&mut self) -> BoxFuture<'_, Result<Option<String>, DriverError>> {
        Box::pin(async move { self.fingerprint() })
    }
    fn object_detail<'a>(
        &'a mut self,
        object: &'a ObjectIdentity,
    ) -> BoxFuture<'a, Result<ObjectDetail, DriverError>> {
        Box::pin(async move { self.inspect(object) })
    }
}
