// SPDX-License-Identifier: Apache-2.0
//! `SQLite` guarded edit SQL runs atomically on one BEGIN IMMEDIATE baton.
use crate::LibsqlSession;
use crate::catalog::qualified;
use openrows_driver_api::{BoxFuture, DriverError, Editor, RowEditInfo, RowSnapshot};
use openrows_driver_sqlite::catalog_sql;
use openrows_sql::{Dialect, select::BoundSql};

impl LibsqlSession {
    async fn edit_info(&mut self, schema: &str, table: &str) -> Result<RowEditInfo, DriverError> {
        let columns = self.columns(schema, table).await?;
        let mut keyed = columns
            .iter()
            .filter(|column| column.pk > 0)
            .collect::<Vec<_>>();
        keyed.sort_by_key(|column| column.pk);
        let mut key = keyed
            .into_iter()
            .map(|column| column.detail.name.clone())
            .collect::<Vec<_>>();
        if key.is_empty() {
            let not_null = columns
                .iter()
                .filter(|column| !column.detail.nullable)
                .map(|column| column.detail.name.as_str())
                .collect::<std::collections::HashSet<_>>();
            key = self
                .indexes(schema, table)
                .await?
                .into_iter()
                .filter(|index| index.unique && index.predicate.is_none())
                .map(|index| {
                    index
                        .columns
                        .split(", ")
                        .map(str::to_owned)
                        .collect::<Vec<_>>()
                })
                .find(|names| {
                    !names.is_empty() && names.iter().all(|name| not_null.contains(name.as_str()))
                })
                .unwrap_or_default();
        }
        let mut protected = columns
            .iter()
            .filter(|column| column.hidden >= 2)
            .map(|column| column.detail.name.clone())
            .collect::<Vec<_>>();
        let row_id = if key.is_empty() {
            self.row_id(schema, table, &columns).await?
        } else {
            None
        };
        if let Some(name) = &row_id {
            key.push(name.clone());
            protected.push(name.clone());
        }
        Ok(RowEditInfo {
            key,
            protected,
            row_id,
        })
    }

    async fn row_id(
        &mut self,
        schema: &str,
        table: &str,
        columns: &[crate::catalog::Column],
    ) -> Result<Option<String>, DriverError> {
        if table.to_ascii_lowercase().starts_with("sqlite_") {
            return Ok(None);
        }
        let rows = self
            .text_query(
                catalog_sql::ROWID_TABLE,
                &[Some(schema.into()), Some(table.into())],
            )
            .await?;
        let eligible = rows.first().is_some_and(|row| {
            row.first().and_then(|value| value.as_deref()) == Some("table")
                && row.get(1).and_then(|value| value.as_deref()) == Some("0")
        });
        Ok(eligible
            .then(|| {
                ["rowid", "_rowid_", "oid"]
                    .into_iter()
                    .find(|alias| {
                        !columns
                            .iter()
                            .any(|column| column.detail.name.eq_ignore_ascii_case(alias))
                    })
                    .map(str::to_owned)
            })
            .flatten())
    }

    async fn snapshot(
        &mut self,
        target: (&str, &str),
        keys: &[(String, String)],
    ) -> Result<RowSnapshot, DriverError> {
        if keys.is_empty() {
            return Err(DriverError::Unsupported("A stable key is required".into()));
        }
        let columns = self.columns(target.0, target.1).await?;
        let row_id = self
            .row_id(target.0, target.1, &columns)
            .await?
            .filter(|name| keys.iter().any(|(key, _)| key == name));
        let names = columns
            .into_iter()
            .map(|column| column.detail.name)
            .chain(row_id)
            .collect::<Vec<_>>();
        let quote = |name: &str| {
            Dialect::Sqlite
                .quote_ident(name)
                .map_err(|error| DriverError::Unsupported(error.to_string()))
        };
        let projection = names
            .iter()
            .map(|name| quote(name).map(|name| Dialect::Sqlite.text_of(&name)))
            .collect::<Result<Vec<_>, _>>()?
            .join(", ");
        let predicates = keys
            .iter()
            .enumerate()
            .map(|(index, (name, _))| quote(name).map(|name| format!("{name} = ?{}", index + 1)))
            .collect::<Result<Vec<_>, _>>()?
            .join(" AND ");
        let relation = qualified(target.0, target.1)?;
        let params = keys
            .iter()
            .map(|(_, value)| Some(value.clone()))
            .collect::<Vec<_>>();
        let mut rows = self
            .text_query(
                &format!("SELECT {projection} FROM {relation} WHERE {predicates} LIMIT 2"),
                &params,
            )
            .await?;
        if rows.len() != 1 {
            return Err(DriverError::Unsupported(
                "The row changed or no longer has a unique identity; reload it".into(),
            ));
        }
        Ok(names.into_iter().zip(rows.remove(0)).collect())
    }

    async fn commit(&mut self, statements: &[BoundSql], one_row: bool) -> Result<(), DriverError> {
        if self.in_transaction() {
            return Err(DriverError::Unsupported(
                "finish the current transaction before committing edits".into(),
            ));
        }
        self.control("BEGIN IMMEDIATE", &[]).await?;
        for statement in statements {
            let changed = self.control(&statement.sql, &statement.params).await;
            match changed {
                Ok(changed) if !one_row || changed == 1 => {}
                result => {
                    self.rollback().await?;
                    return Err(result.err().unwrap_or_else(conflict));
                }
            }
        }
        match self.control("COMMIT", &[]).await {
            Ok(_) => Ok(()),
            Err(error)
                if !self.usable()
                    || error.is_connection_lost()
                    || matches!(&error, DriverError::Server { code, .. } if code == "57014") =>
            {
                Err(DriverError::Server {
                    code: "08007".into(),
                    message: "libSQL COMMIT outcome is unknown; reload before retrying".into(),
                })
            }
            Err(error) => {
                self.rollback().await?;
                Err(error)
            }
        }
    }

    async fn rollback(&mut self) -> Result<(), DriverError> {
        self.control("ROLLBACK", &[]).await.map(drop)
    }
}

impl Editor for LibsqlSession {
    fn row_edit_info<'a>(
        &'a mut self,
        schema: &'a str,
        table: &'a str,
    ) -> BoxFuture<'a, Result<RowEditInfo, DriverError>> {
        Box::pin(self.edit_info(schema, table))
    }
    fn edit_snapshot<'a>(
        &'a mut self,
        target: (&'a str, &'a str),
        keys: &'a [(String, String)],
    ) -> BoxFuture<'a, Result<RowSnapshot, DriverError>> {
        Box::pin(self.snapshot(target, keys))
    }
    fn commit_edits<'a>(
        &'a mut self,
        edits: &'a [BoundSql],
    ) -> BoxFuture<'a, Result<(), DriverError>> {
        Box::pin(self.commit(edits, true))
    }
    fn commit_ddl<'a>(
        &'a mut self,
        statements: &'a [String],
    ) -> BoxFuture<'a, Result<(), DriverError>> {
        let bound = statements
            .iter()
            .map(|sql| BoundSql {
                sql: sql.clone(),
                params: Vec::new(),
            })
            .collect::<Vec<_>>();
        Box::pin(async move { self.commit(&bound, false).await })
    }
}
fn conflict() -> DriverError {
    DriverError::Server { code: "40001".into(), message: "A row changed, was deleted, or matched more than once. All changes were rolled back; reload and review.".into() }
}
use openrows_driver_api::Session as _;
