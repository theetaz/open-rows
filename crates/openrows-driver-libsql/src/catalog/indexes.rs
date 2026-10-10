// SPDX-License-Identifier: Apache-2.0
//! Indexes and stored definitions through the shared `SQLite` pragma queries.
use super::text::{integer, optional, qualified, value};
use crate::LibsqlSession;
use openrows_driver_api::{DriverError, schema::IndexDetail};
use openrows_driver_sqlite::catalog_sql;

impl LibsqlSession {
    pub(crate) async fn indexes(
        &mut self,
        schema: &str,
        table: &str,
    ) -> Result<Vec<IndexDetail>, DriverError> {
        let rows = self
            .text_query(
                catalog_sql::INDEXES,
                &[Some(table.into()), Some(schema.into())],
            )
            .await?;
        let mut indexes = Vec::new();
        for row in rows {
            let name = value(&row, 0)?;
            let origin = value(&row, 2)?;
            let definition = self.stored_sql(schema, "index", &name).await?;
            let columns = self
                .text_query(
                    catalog_sql::INDEX_COLUMNS,
                    &[Some(name.clone()), Some(schema.into())],
                )
                .await?
                .iter()
                .map(|row| value(row, 0))
                .collect::<Result<Vec<_>, _>>()?
                .join(", ");
            let predicate = if integer(&row, 3)? != 0 {
                predicate(definition.as_deref())
            } else {
                None
            };
            indexes.push(IndexDetail {
                name: name.clone(),
                unique: integer(&row, 1)? != 0,
                primary: origin == "pk",
                constraint: origin != "c",
                method: "btree".into(),
                columns,
                include: Vec::new(),
                predicate,
                comment: None,
                definition: definition.unwrap_or_else(|| format!("-- automatic index {name}")),
            });
        }
        Ok(indexes)
    }

    pub(crate) async fn stored_sql(
        &mut self,
        schema: &str,
        kind: &str,
        name: &str,
    ) -> Result<Option<String>, DriverError> {
        let master = qualified(schema, "sqlite_master")?;
        let sql = format!("SELECT sql FROM {master} WHERE type = ?1 AND name = ?2");
        let rows = self
            .text_query(&sql, &[Some(kind.into()), Some(name.into())])
            .await?;
        Ok(rows.first().and_then(|row| optional(row, 0)))
    }
}
fn predicate(definition: Option<&str>) -> Option<String> {
    let text = definition?;
    let index = text.to_ascii_uppercase().rfind(" WHERE ")?;
    text.get(index + 7..).map(|text| text.trim().into())
}
