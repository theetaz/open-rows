// SPDX-License-Identifier: Apache-2.0
use crate::{MongoSession, error};
use openrows_driver_api::catalog::{
    ColumnSummary, DatabaseSummary, IndexSummary, ObjectKind, ObjectSummary, SchemaSummary,
    TableStructure,
};
use openrows_driver_api::schema::{ColumnDetail, Generated, IndexDetail, TableDetail};
use openrows_driver_api::{BoxFuture, Catalog, DriverError};

impl MongoSession {
    async fn objects(&self, kinds: &[ObjectKind]) -> Result<Vec<ObjectSummary>, DriverError> {
        let mut cursor = self
            .database()
            .list_collections()
            .await
            .map_err(error::mongo)?;
        let mut objects = Vec::new();
        while cursor.advance().await.map_err(error::mongo)? {
            let specification = cursor.deserialize_current().map_err(error::mongo)?;
            let kind = if specification.collection_type == mongodb::results::CollectionType::View {
                ObjectKind::View
            } else {
                ObjectKind::Table
            };
            if kinds.contains(&kind) {
                objects.push(ObjectSummary {
                    schema: self.summary.database.clone(),
                    name: specification.name,
                    kind,
                    arguments: None,
                    row_estimate: None,
                    comment: None,
                });
            }
        }
        objects.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(objects)
    }
    pub(crate) async fn is_view(&self, table: &str) -> Result<bool, DriverError> {
        let mut cursor = self
            .database()
            .list_collections()
            .filter(mongodb::bson::doc! {"name":table})
            .await
            .map_err(error::mongo)?;
        if !cursor.advance().await.map_err(error::mongo)? {
            return Ok(false);
        }
        Ok(cursor
            .deserialize_current()
            .map_err(error::mongo)?
            .collection_type
            == mongodb::results::CollectionType::View)
    }
    async fn indexes(&self, table: &str) -> Result<Vec<IndexSummary>, DriverError> {
        if self.is_view(table).await? {
            return Ok(Vec::new());
        }
        let mut cursor = self
            .database()
            .collection::<mongodb::bson::Document>(table)
            .list_indexes()
            .await
            .map_err(error::mongo)?;
        let mut indexes = Vec::new();
        while cursor.advance().await.map_err(error::mongo)? {
            let index = cursor.deserialize_current().map_err(error::mongo)?;
            let options = index.options.unwrap_or_default();
            let name = options.name.unwrap_or_default();
            let primary = name == "_id_";
            indexes.push(IndexSummary {
                primary,
                name,
                columns: index.keys.to_string(),
                unique: options.unique.unwrap_or(primary),
                method: "MongoDB".into(),
                predicate: options.partial_filter_expression.map(|v| v.to_string()),
            });
        }
        Ok(indexes)
    }
}
impl Catalog for MongoSession {
    fn list_databases(&mut self) -> BoxFuture<'_, Result<Vec<DatabaseSummary>, DriverError>> {
        Box::pin(async move {
            Ok(self
                .client
                .list_database_names()
                .await
                .map_err(error::mongo)?
                .into_iter()
                .map(|name| DatabaseSummary {
                    system: matches!(name.as_str(), "admin" | "config" | "local"),
                    name,
                    comment: None,
                })
                .collect())
        })
    }
    fn list_schemas(&mut self) -> BoxFuture<'_, Result<Vec<SchemaSummary>, DriverError>> {
        Box::pin(async move {
            Ok(vec![SchemaSummary {
                name: self.summary.database.clone(),
                system: false,
                comment: None,
            }])
        })
    }
    fn list_objects<'a>(
        &'a mut self,
        _schema: &'a str,
        kinds: &'a [ObjectKind],
    ) -> BoxFuture<'a, Result<Vec<ObjectSummary>, DriverError>> {
        Box::pin(self.objects(kinds))
    }
    fn search_index(&mut self) -> BoxFuture<'_, Result<Vec<ObjectSummary>, DriverError>> {
        Box::pin(self.objects(&[ObjectKind::Table, ObjectKind::View]))
    }
    fn table_columns<'a>(
        &'a mut self,
        _schema: &'a str,
        _table: &'a str,
    ) -> BoxFuture<'a, Result<Vec<ColumnSummary>, DriverError>> {
        Box::pin(async {
            Ok(["_id", "__document"]
                .into_iter()
                .map(|name| ColumnSummary {
                    name: name.into(),
                    data_type: "EJSON".into(),
                    nullable: false,
                    default: None,
                    comment: None,
                    primary_key: name == "_id",
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
            Ok(TableStructure {
                columns: self.table_columns(schema, table).await?,
                indexes: self.indexes(table).await?,
                row_estimate: None,
            })
        })
    }
    fn table_detail<'a>(
        &'a mut self,
        schema: &'a str,
        table: &'a str,
    ) -> BoxFuture<'a, Result<TableDetail, DriverError>> {
        Box::pin(async move {
            let structure = self.table_structure(schema, table).await?;
            Ok(TableDetail {
                columns: structure
                    .columns
                    .into_iter()
                    .map(|column| ColumnDetail {
                        name: column.name,
                        data_type: column.data_type,
                        nullable: column.nullable,
                        default: None,
                        generated: Generated::No,
                        comment: None,
                    })
                    .collect(),
                indexes: structure
                    .indexes
                    .into_iter()
                    .map(|index| IndexDetail {
                        definition: format!(
                            "db.getCollection({}).getIndexes()",
                            serde_json::json!(table)
                        ),
                        name: index.name,
                        method: index.method,
                        unique: index.unique,
                        primary: index.primary,
                        constraint: index.primary,
                        columns: index.columns,
                        include: Vec::new(),
                        predicate: index.predicate,
                        comment: None,
                    })
                    .collect(),
                ..TableDetail::default()
            })
        })
    }
}
