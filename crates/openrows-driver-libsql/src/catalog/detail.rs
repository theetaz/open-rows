// SPDX-License-Identifier: Apache-2.0
//! Full table detail and view inspection use the same `SQLite` stored definitions.
use super::{
    Column,
    text::{integer, optional, qualified, value},
};
use crate::LibsqlSession;
use openrows_driver_api::{
    DriverError,
    schema::{
        ConstraintDetail, ConstraintKind, ForeignTarget, IndexDetail, RelationKind, TableDetail,
        TableInfo,
    },
};
use openrows_driver_sqlite::{catalog_definition, catalog_sql};

impl LibsqlSession {
    pub(super) async fn detail(
        &mut self,
        schema: &str,
        table: &str,
    ) -> Result<TableDetail, DriverError> {
        let master = qualified(schema, "sqlite_master")?;
        let sql =
            format!("SELECT type, sql FROM {master} WHERE name = ?1 AND type IN ('table', 'view')");
        let found = self.text_query(&sql, &[Some(table.into())]).await?;
        let Some(row) = found.first() else {
            return Ok(TableDetail::default());
        };
        let view = value(row, 0)? == "view";
        let create = optional(row, 1);
        let columns = self.columns(schema, table).await?;
        let indexes = self.indexes(schema, table).await?;
        let sql = format!(
            "SELECT name, sql FROM {master} WHERE type = 'trigger' AND tbl_name = ?1 ORDER BY name"
        );
        let triggers = self
            .text_query(&sql, &[Some(table.into())])
            .await?
            .iter()
            .map(|row| {
                Ok(catalog_definition::trigger(
                    value(row, 0)?,
                    optional(row, 1).unwrap_or_default(),
                ))
            })
            .collect::<Result<Vec<_>, DriverError>>()?;
        let mut constraints = primary(&columns).into_iter().collect::<Vec<_>>();
        constraints.extend(unique(&indexes));
        constraints.extend(self.foreign_keys(schema, table).await?);
        constraints.extend(catalog_definition::checks(table, create.as_deref()));
        let definition = catalog_definition::definition(create.as_deref(), &indexes, &triggers);
        Ok(TableDetail {
            columns: columns.into_iter().map(|column| column.detail).collect(),
            constraints,
            indexes,
            triggers,
            partitions: Vec::new(),
            info: Some(TableInfo {
                kind: if view {
                    RelationKind::View
                } else {
                    RelationKind::Table
                },
                view_definition: if view { create } else { None },
                definition: Some(definition),
                ..TableInfo::default()
            }),
        })
    }

    async fn foreign_keys(
        &mut self,
        schema: &str,
        table: &str,
    ) -> Result<Vec<ConstraintDetail>, DriverError> {
        let rows = self
            .text_query(
                catalog_sql::FOREIGN_KEYS,
                &[Some(table.into()), Some(schema.into())],
            )
            .await?;
        let mut constraints: Vec<ConstraintDetail> = Vec::new();
        for row in rows {
            let name = format!("fk_{table}_{}", integer(&row, 0)?);
            if constraints.last().is_none_or(|key| key.name != name) {
                constraints.push(ConstraintDetail {
                    name,
                    kind: ConstraintKind::Foreign,
                    definition: String::new(),
                    columns: Vec::new(),
                    references: Some(ForeignTarget {
                        schema: schema.into(),
                        table: value(&row, 1)?,
                        columns: Vec::new(),
                    }),
                });
            }
            if let Some(key) = constraints.last_mut() {
                append_foreign_columns(key, &row)?;
            }
        }
        for key in &mut constraints {
            if let Some(target) = &key.references {
                key.definition = format!(
                    "FOREIGN KEY ({}) REFERENCES {} ({})",
                    key.columns.join(", "),
                    target.table,
                    target.columns.join(", ")
                );
            }
        }
        Ok(constraints)
    }
}
fn append_foreign_columns(
    key: &mut ConstraintDetail,
    row: &[Option<String>],
) -> Result<(), DriverError> {
    key.columns.push(value(row, 2)?);
    if let Some(target) = &mut key.references {
        target.columns.push(optional(row, 3).unwrap_or_default());
    }
    Ok(())
}
fn primary(columns: &[Column]) -> Option<ConstraintDetail> {
    let mut key = columns
        .iter()
        .filter(|column| column.pk > 0)
        .collect::<Vec<_>>();
    key.sort_by_key(|column| column.pk);
    let columns = key
        .into_iter()
        .map(|column| column.detail.name.clone())
        .collect::<Vec<_>>();
    (!columns.is_empty()).then(|| ConstraintDetail {
        name: "PRIMARY KEY".into(),
        kind: ConstraintKind::Primary,
        definition: format!("PRIMARY KEY ({})", columns.join(", ")),
        columns,
        references: None,
    })
}
fn unique(indexes: &[IndexDetail]) -> Vec<ConstraintDetail> {
    indexes
        .iter()
        .filter(|index| index.unique && index.constraint && !index.primary)
        .map(|index| ConstraintDetail {
            name: index.name.clone(),
            kind: ConstraintKind::Unique,
            definition: format!("UNIQUE ({})", index.columns),
            columns: index.columns.split(", ").map(str::to_owned).collect(),
            references: None,
        })
        .collect()
}
