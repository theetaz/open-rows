// SPDX-License-Identifier: Apache-2.0
//! `ClickHouse` system catalogs; every identity is a bound parameter.
use crate::session::ClickHouseSession;
use openrows_driver_api::catalog::{
    ColumnSummary, DatabaseSummary, IndexSummary, ObjectKind, ObjectSummary, SchemaSummary,
    TableStructure,
};
use openrows_driver_api::inspection::{ObjectDetail, ObjectIdentity, view_detail};
use openrows_driver_api::schema::{
    ColumnDetail, Generated, IndexDetail, RelationKind, TableDetail, TableInfo,
};
use openrows_driver_api::{BoxFuture, Catalog, DriverError};
use serde_json::Value;

pub(crate) fn text(row: &[Value], index: usize) -> String {
    row.get(index).map_or_else(String::new, |v| {
        v.as_str().map_or_else(
            || {
                if v.is_null() {
                    String::new()
                } else {
                    v.to_string()
                }
            },
            str::to_owned,
        )
    })
}
fn optional(row: &[Value], index: usize) -> Option<String> {
    let value = text(row, index);
    (!value.is_empty()).then_some(value)
}
fn number(row: &[Value], index: usize) -> Option<u64> {
    text(row, index).parse().ok()
}
fn params(schema: &str, table: &str) -> [Option<String>; 2] {
    [Some(schema.to_owned()), Some(table.to_owned())]
}
fn column(row: &[Value]) -> ColumnSummary {
    let kind = text(row, 1);
    ColumnSummary {
        name: text(row, 0),
        nullable: kind.starts_with("Nullable(") || kind.starts_with("LowCardinality(Nullable("),
        data_type: kind,
        default: optional(row, 3),
        comment: optional(row, 4),
        primary_key: text(row, 5) == "1",
        enum_labels: None,
    }
}
fn objects(rows: Vec<Vec<Value>>) -> Vec<ObjectSummary> {
    rows.into_iter()
        .filter_map(|row| {
            Some(ObjectSummary {
                schema: text(&row, 0),
                name: text(&row, 1),
                kind: ObjectKind::parse(&text(&row, 2))?,
                arguments: None,
                row_estimate: number(&row, 3),
                comment: optional(&row, 4),
            })
        })
        .collect()
}
impl ClickHouseSession {
    async fn columns(&self, target: (&str, &str)) -> Result<Vec<Vec<Value>>, DriverError> {
        self.text_query(include_str!("sql/columns.sql"), &params(target.0, target.1))
            .await
    }
    async fn detail(&self, target: (&str, &str)) -> Result<TableDetail, DriverError> {
        let rows = self.columns(target).await?;
        let columns = rows
            .iter()
            .map(|row| {
                let summary = column(row);
                ColumnDetail {
                    name: summary.name,
                    data_type: summary.data_type,
                    nullable: summary.nullable,
                    default: summary.default,
                    comment: summary.comment,
                    generated: if text(row, 2) == "MATERIALIZED" {
                        Generated::Stored
                    } else {
                        Generated::No
                    },
                }
            })
            .collect();
        let rows = self
            .text_query(include_str!("sql/table.sql"), &params(target.0, target.1))
            .await?;
        let info = rows.first().map(|row| {
            let kind = match text(row, 0).as_str() {
                "View" => RelationKind::View,
                "MaterializedView" => RelationKind::MaterializedView,
                _ => RelationKind::Table,
            };
            TableInfo {
                kind,
                row_estimate: number(row, 1),
                total_bytes: number(row, 2).unwrap_or(0),
                data_bytes: number(row, 2).unwrap_or(0),
                definition: optional(row, 3),
                comment: optional(row, 4),
                ..TableInfo::default()
            }
        });
        let rows = self
            .text_query(include_str!("sql/indexes.sql"), &params(target.0, target.1))
            .await?;
        let indexes = rows
            .iter()
            .map(|row| IndexDetail {
                name: text(row, 0),
                columns: text(row, 1),
                method: text(row, 2),
                definition: format!(
                    "INDEX {} {} TYPE {}",
                    text(row, 0),
                    text(row, 1),
                    text(row, 2)
                ),
                ..IndexDetail::default()
            })
            .collect();
        Ok(TableDetail {
            columns,
            indexes,
            info,
            ..TableDetail::default()
        })
    }
}
impl Catalog for ClickHouseSession {
    fn list_databases(&mut self) -> BoxFuture<'_, Result<Vec<DatabaseSummary>, DriverError>> {
        Box::pin(async move {
            Ok(self
                .text_query(include_str!("sql/databases.sql"), &[])
                .await?
                .iter()
                .map(|row| {
                    let name = text(row, 0);
                    DatabaseSummary {
                        system: matches!(
                            name.as_str(),
                            "system" | "information_schema" | "INFORMATION_SCHEMA"
                        ),
                        name,
                        comment: None,
                    }
                })
                .collect())
        })
    }
    fn list_schemas(&mut self) -> BoxFuture<'_, Result<Vec<SchemaSummary>, DriverError>> {
        Box::pin(async move {
            Ok(vec![SchemaSummary {
                name: self.summary.database.clone(),
                system: self.summary.database == "system",
                comment: None,
            }])
        })
    }
    fn list_objects<'a>(
        &'a mut self,
        schema: &'a str,
        kinds: &'a [ObjectKind],
    ) -> BoxFuture<'a, Result<Vec<ObjectSummary>, DriverError>> {
        Box::pin(async move {
            Ok(objects(
                self.text_query(include_str!("sql/objects.sql"), &[Some(schema.to_owned())])
                    .await?,
            )
            .into_iter()
            .filter(|object| kinds.contains(&object.kind))
            .collect())
        })
    }
    fn search_index(&mut self) -> BoxFuture<'_, Result<Vec<ObjectSummary>, DriverError>> {
        Box::pin(async move {
            Ok(objects(
                self.text_query(
                    include_str!("sql/objects.sql"),
                    &[Some(self.summary.database.clone())],
                )
                .await?,
            ))
        })
    }
    fn table_columns<'a>(
        &'a mut self,
        schema: &'a str,
        table: &'a str,
    ) -> BoxFuture<'a, Result<Vec<ColumnSummary>, DriverError>> {
        Box::pin(async move {
            Ok(self
                .columns((schema, table))
                .await?
                .iter()
                .map(|row| column(row))
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
            let detail = self.detail((schema, table)).await?;
            Ok(TableStructure {
                columns,
                row_estimate: detail.info.and_then(|i| i.row_estimate),
                indexes: detail
                    .indexes
                    .into_iter()
                    .map(|i| IndexSummary {
                        name: i.name,
                        columns: i.columns,
                        method: i.method,
                        unique: false,
                        primary: false,
                        predicate: None,
                    })
                    .collect(),
            })
        })
    }
    fn table_detail<'a>(
        &'a mut self,
        schema: &'a str,
        table: &'a str,
    ) -> BoxFuture<'a, Result<TableDetail, DriverError>> {
        Box::pin(self.detail((schema, table)))
    }
    fn object_detail<'a>(
        &'a mut self,
        object: &'a ObjectIdentity,
    ) -> BoxFuture<'a, Result<ObjectDetail, DriverError>> {
        Box::pin(async move {
            match object.kind {
                ObjectKind::View | ObjectKind::MaterializedView => {
                    view_detail(self.detail((&object.schema, &object.name)).await?)
                }
                ObjectKind::Function => {
                    let rows = self
                        .text_query(
                            include_str!("sql/function.sql"),
                            &[Some(object.name.clone())],
                        )
                        .await?;
                    Ok(ObjectDetail {
                        definition: rows.first().and_then(|row| optional(row, 0)),
                        ..ObjectDetail::default()
                    })
                }
                _ => Err(DriverError::Unsupported(
                    "ClickHouse object inspection for this kind".to_owned(),
                )),
            }
        })
    }
}
