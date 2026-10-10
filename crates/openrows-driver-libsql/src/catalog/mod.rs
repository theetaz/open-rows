// SPDX-License-Identifier: Apache-2.0
//! `SQLite` catalog SQL over Hrana, including M2-09 view inspection.
mod detail;
mod indexes;
mod text;
use crate::LibsqlSession;
use openrows_driver_api::catalog::{
    ColumnSummary, DatabaseSummary, IndexSummary, ObjectKind, ObjectSummary, SchemaSummary,
    TableStructure,
};
use openrows_driver_api::schema::{ColumnDetail, TableDetail};
use openrows_driver_api::{BoxFuture, Catalog, DriverError};
use openrows_driver_sqlite::catalog_sql;
pub(crate) use text::qualified;
use text::{integer, optional, value};

pub(crate) struct Column {
    pub(crate) detail: ColumnDetail,
    pub(crate) pk: i64,
    pub(crate) hidden: i64,
}

impl LibsqlSession {
    pub(crate) async fn columns(
        &mut self,
        schema: &str,
        table: &str,
    ) -> Result<Vec<Column>, DriverError> {
        let params = [Some(table.into()), Some(schema.into())];
        self.text_query(catalog_sql::COLUMNS, &params)
            .await?
            .into_iter()
            .map(|row| {
                let pk = integer(&row, 4)?;
                let hidden = integer(&row, 5)?;
                Ok(Column {
                    pk,
                    hidden,
                    detail: ColumnDetail {
                        name: value(&row, 0)?,
                        data_type: value(&row, 1)?,
                        nullable: integer(&row, 2)? == 0 && pk == 0,
                        default: optional(&row, 3),
                        generated: if hidden >= 2 {
                            openrows_driver_api::schema::Generated::Stored
                        } else {
                            openrows_driver_api::schema::Generated::No
                        },
                        comment: None,
                    },
                })
            })
            .collect()
    }

    async fn objects(
        &mut self,
        schema: &str,
        kinds: &[ObjectKind],
    ) -> Result<Vec<ObjectSummary>, DriverError> {
        let master = qualified(schema, "sqlite_master")?;
        let sql = format!(
            "SELECT type, name FROM {master} WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite\\_%' ESCAPE '\\' ORDER BY name"
        );
        self.text_query(&sql, &[])
            .await?
            .into_iter()
            .filter_map(|row| {
                let result = (|| {
                    let kind = match value(&row, 0)?.as_str() {
                        "table" => ObjectKind::Table,
                        "view" => ObjectKind::View,
                        _ => return Err(crate::error::protocol()),
                    };
                    Ok(ObjectSummary {
                        schema: schema.into(),
                        name: value(&row, 1)?,
                        kind,
                        arguments: None,
                        row_estimate: None,
                        comment: None,
                    })
                })();
                match result {
                    Ok(object) if !kinds.contains(&object.kind) => None,
                    other => Some(other),
                }
            })
            .collect()
    }
}

impl Catalog for LibsqlSession {
    fn list_databases(&mut self) -> BoxFuture<'_, Result<Vec<DatabaseSummary>, DriverError>> {
        Box::pin(async {
            Ok(vec![DatabaseSummary {
                name: "main".into(),
                system: false,
                comment: None,
            }])
        })
    }
    fn list_schemas(&mut self) -> BoxFuture<'_, Result<Vec<SchemaSummary>, DriverError>> {
        Box::pin(async move {
            self.text_query(catalog_sql::SCHEMAS, &[])
                .await?
                .into_iter()
                .map(|row| {
                    let name = value(&row, 0)?;
                    Ok(SchemaSummary {
                        system: name == "temp",
                        name,
                        comment: None,
                    })
                })
                .collect()
        })
    }
    fn list_objects<'a>(
        &'a mut self,
        schema: &'a str,
        kinds: &'a [ObjectKind],
    ) -> BoxFuture<'a, Result<Vec<ObjectSummary>, DriverError>> {
        Box::pin(self.objects(schema, kinds))
    }
    fn search_index(&mut self) -> BoxFuture<'_, Result<Vec<ObjectSummary>, DriverError>> {
        Box::pin(async move {
            let mut objects = Vec::new();
            for schema in self.list_schemas().await? {
                objects.extend(
                    self.objects(&schema.name, &[ObjectKind::Table, ObjectKind::View])
                        .await?,
                );
            }
            if objects.len() > 50_000 {
                return Err(DriverError::Unsupported(
                    "libSQL catalog exceeds 50000 objects".into(),
                ));
            }
            Ok(objects)
        })
    }
    fn table_columns<'a>(
        &'a mut self,
        schema: &'a str,
        table: &'a str,
    ) -> BoxFuture<'a, Result<Vec<ColumnSummary>, DriverError>> {
        Box::pin(async move {
            Ok(self
                .columns(schema, table)
                .await?
                .into_iter()
                .map(|column| ColumnSummary {
                    name: column.detail.name,
                    data_type: column.detail.data_type,
                    nullable: column.detail.nullable,
                    default: column.detail.default,
                    comment: None,
                    primary_key: column.pk > 0,
                    enum_labels: None,
                })
                .collect())
        })
    }
    fn table_structure<'a>(
        &'a mut self,
        schema: &'a str,
        table: &'a str,
    ) -> BoxFuture<'a, Result<TableStructure, DriverError>> {
        Box::pin(async move {
            let columns = self.table_columns(schema, table).await?;
            let indexes = self
                .indexes(schema, table)
                .await?
                .into_iter()
                .map(|index| IndexSummary {
                    name: index.name,
                    columns: index.columns,
                    unique: index.unique,
                    primary: index.primary,
                    method: index.method,
                    predicate: index.predicate,
                })
                .collect();
            Ok(TableStructure {
                columns,
                indexes,
                row_estimate: None,
            })
        })
    }
    fn table_detail<'a>(
        &'a mut self,
        schema: &'a str,
        table: &'a str,
    ) -> BoxFuture<'a, Result<TableDetail, DriverError>> {
        Box::pin(self.detail(schema, table))
    }
    fn object_detail<'a>(
        &'a mut self,
        object: &'a openrows_driver_api::inspection::ObjectIdentity,
    ) -> BoxFuture<'a, Result<openrows_driver_api::inspection::ObjectDetail, DriverError>> {
        Box::pin(async move {
            if object.kind != ObjectKind::View {
                return Err(DriverError::Unsupported("this libSQL object viewer".into()));
            }
            let detail = self.detail(&object.schema, &object.name).await?;
            openrows_driver_api::inspection::view_detail(detail)
        })
    }
}
