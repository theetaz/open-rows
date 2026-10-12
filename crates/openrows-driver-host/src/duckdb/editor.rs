// SPDX-License-Identifier: Apache-2.0
//! Guarded edits on the shared session: key-based, one transaction, never
//! inside a transaction the user opened on the same connection.
use super::catalog_sql as sql;
use super::error;
use super::session::DuckdbSession;
use super::text;
use openrows_driver_api::{BoxFuture, DriverError, Editor, RowEditInfo, RowSnapshot, Session};
use openrows_sql::{Dialect, select::BoundSql};
use std::collections::HashSet;

impl DuckdbSession {
    fn edit_info(&self, schema: &str, table: &str) -> Result<RowEditInfo, DriverError> {
        let not_null = self
            .columns(schema, table)?
            .into_iter()
            .filter(|column| !column.nullable)
            .map(|column| column.name)
            .collect::<HashSet<_>>();
        let mut keys: Vec<(String, bool, Vec<String>)> = Vec::new();
        for row in self.query(sql::KEYS, &[Some(schema), Some(table)], 3)? {
            let position = text::value(&row, 0)?;
            let column = text::value(&row, 2)?;
            match keys.last_mut() {
                Some((last, _, columns)) if *last == position => columns.push(column),
                _ => keys.push((
                    position,
                    text::value(&row, 1)? == "PRIMARY KEY",
                    vec![column],
                )),
            }
        }
        let key = keys
            .into_iter()
            .find(|(_, primary, columns)| {
                *primary || columns.iter().all(|name| not_null.contains(name))
            })
            .map(|(_, _, columns)| columns)
            .unwrap_or_default();
        Ok(RowEditInfo {
            key,
            protected: Vec::new(),
            row_id: None,
        })
    }

    fn snapshot(
        &self,
        (schema, table): (&str, &str),
        keys: &[(String, String)],
    ) -> Result<RowSnapshot, DriverError> {
        if keys.is_empty() {
            return Err(DriverError::Unsupported("A stable key is required".into()));
        }
        let names = self
            .columns(schema, table)?
            .into_iter()
            .map(|column| column.name)
            .collect::<Vec<_>>();
        let select = snapshot_sql((schema, table), &names, keys)?;
        let params = keys
            .iter()
            .map(|(_, value)| Some(value.as_str()))
            .collect::<Vec<_>>();
        let mut rows = self.query(&select, &params, names.len())?;
        if rows.len() != 1 {
            return Err(DriverError::Unsupported(
                "The row changed or no longer has a unique identity; reload it".into(),
            ));
        }
        Ok(names.into_iter().zip(rows.remove(0)).collect())
    }

    /// Run `statements` in one transaction; with `one_row`, each must change exactly one row.
    fn commit(&self, statements: &[BoundSql], one_row: bool) -> Result<(), DriverError> {
        if self.in_transaction() {
            return Err(DriverError::Unsupported(
                "Commit or roll back the open transaction before committing edits".into(),
            ));
        }
        if statements
            .iter()
            .any(|statement| openrows_sql::duckdb::has_attachment(&statement.sql))
        {
            return Err(DriverError::Unsupported(
                "DuckDB ATTACH is unavailable until attached-file reservations are supported"
                    .into(),
            ));
        }
        self.native
            .execute_batch("BEGIN TRANSACTION")
            .map_err(error::map)?;
        for statement in statements {
            let params = statement.params.iter().map(Option::as_deref);
            match self
                .native
                .execute(&statement.sql, duckdb::params_from_iter(params))
            {
                Ok(changed) if !one_row || changed == 1 => {}
                result => {
                    self.rollback();
                    return Err(result.err().map_or_else(conflict, error::map));
                }
            }
        }
        self.native.execute_batch("COMMIT").map_err(|cause| {
            self.rollback();
            error::map(cause)
        })
    }

    /// Best effort: an aborted `DuckDB` transaction must still be ended.
    fn rollback(&self) {
        let _ = self.native.execute_batch("ROLLBACK");
    }
}

impl Editor for DuckdbSession {
    fn row_edit_info<'a>(
        &'a mut self,
        schema: &'a str,
        table: &'a str,
    ) -> BoxFuture<'a, Result<RowEditInfo, DriverError>> {
        Box::pin(async move { self.edit_info(schema, table) })
    }
    fn edit_snapshot<'a>(
        &'a mut self,
        target: (&'a str, &'a str),
        keys: &'a [(String, String)],
    ) -> BoxFuture<'a, Result<RowSnapshot, DriverError>> {
        Box::pin(async move { self.snapshot(target, keys) })
    }
    fn commit_edits<'a>(
        &'a mut self,
        edits: &'a [BoundSql],
    ) -> BoxFuture<'a, Result<(), DriverError>> {
        Box::pin(async move { self.commit(edits, true) })
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
        Box::pin(async move { self.commit(&bound, false) })
    }
}

/// Exact-text projection of every column for the row whose key columns equal `$n`.
fn snapshot_sql(
    (schema, table): (&str, &str),
    names: &[String],
    keys: &[(String, String)],
) -> Result<String, DriverError> {
    let quote = |name: &str| {
        Dialect::Duckdb
            .quote_ident(name)
            .map_err(|error| DriverError::Unsupported(error.to_string()))
    };
    let projection = names
        .iter()
        .map(|name| quote(name).map(|name| Dialect::Duckdb.text_of(&name)))
        .collect::<Result<Vec<_>, _>>()?
        .join(", ");
    let predicates = keys
        .iter()
        .enumerate()
        .map(|(index, (name, _))| quote(name).map(|name| format!("{name} = ${}", index + 1)))
        .collect::<Result<Vec<_>, _>>()?
        .join(" AND ");
    let relation = Dialect::Duckdb
        .qualified(schema, table)
        .map_err(|error| DriverError::Unsupported(error.to_string()))?;
    Ok(format!(
        "SELECT {projection} FROM {relation} WHERE {predicates} LIMIT 2"
    ))
}

fn conflict() -> DriverError {
    DriverError::Server {
        code: "40001".into(),
        message: "A row changed, was deleted, or matched more than once. All changes were rolled back; reload and review.".into(),
    }
}
