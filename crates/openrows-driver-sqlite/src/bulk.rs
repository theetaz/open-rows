// SPDX-License-Identifier: Apache-2.0
//! Bulk import (FR-IO-02): one cached prepared `INSERT` executed for every
//! row on the connection thread, inside the caller's transaction. Text
//! values take the column's affinity, as with bound parameters elsewhere.
use openrows_driver_api::{BoxFuture, BulkLoader, BulkTarget, DriverError, TextRow};
use openrows_sql::Dialect;
use rusqlite::{Connection, params_from_iter};

use crate::error::driver_error;
use crate::session::SqliteSession;

/// `INSERT INTO "schema"."table" ("a", "b") VALUES (?1, ?2)`.
fn insert_sql(target: BulkTarget<'_>) -> Result<String, DriverError> {
    let invalid = |_| DriverError::Unsupported("names containing NUL characters".to_owned());
    let table = Dialect::Sqlite
        .qualified(target.schema, target.table)
        .map_err(invalid)?;
    let columns = target
        .columns
        .iter()
        .map(|name| Dialect::Sqlite.quote_ident(name))
        .collect::<Result<Vec<_>, _>>()
        .map_err(invalid)?
        .join(", ");
    let values = (1..=target.columns.len())
        .map(|number| format!("?{number}"))
        .collect::<Vec<_>>()
        .join(", ");
    Ok(format!("INSERT INTO {table} ({columns}) VALUES ({values})"))
}

/// Execute the cached statement once per row; returns the rows stored.
fn insert_rows(connection: &Connection, sql: &str, rows: &[TextRow]) -> Result<u64, DriverError> {
    let mut statement = connection
        .prepare_cached(sql)
        .map_err(|error| driver_error(&error))?;
    let mut stored = 0;
    for row in rows {
        stored += statement
            .execute(params_from_iter(row.iter()))
            .map_err(|error| driver_error(&error))? as u64;
    }
    Ok(stored)
}

impl BulkLoader for SqliteSession {
    fn load<'a>(
        &'a mut self,
        target: BulkTarget<'a>,
        rows: Vec<TextRow>,
    ) -> BoxFuture<'a, Result<u64, DriverError>> {
        Box::pin(async move {
            let sql = insert_sql(target)?;
            self.worker
                .call(move |connection| insert_rows(connection, &sql, &rows))
                .await?
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_insert_through_one_cached_statement() -> Result<(), DriverError> {
        let connection = Connection::open_in_memory().map_err(|error| driver_error(&error))?;
        connection
            .execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT)")
            .map_err(|error| driver_error(&error))?;
        let columns = ["id".to_owned(), "name".to_owned()];
        let sql = insert_sql(BulkTarget {
            schema: "main",
            table: "t",
            columns: &columns,
        })?;
        assert_eq!(
            sql,
            "INSERT INTO \"main\".\"t\" (\"id\", \"name\") VALUES (?1, ?2)"
        );
        let rows = vec![
            vec![Some("1".to_owned()), Some("a".to_owned())],
            vec![Some("2".to_owned()), None],
        ];
        assert_eq!(insert_rows(&connection, &sql, &rows)?, 2);
        let typed: i64 = connection
            .query_row(
                "SELECT typeof(id) = 'integer' FROM t WHERE id = 2",
                [],
                |row| row.get(0),
            )
            .map_err(|error| driver_error(&error))?;
        assert_eq!(typed, 1);
        let duplicate = vec![vec![Some("1".to_owned()), None]];
        assert!(matches!(
            insert_rows(&connection, &sql, &duplicate),
            Err(DriverError::Server { .. })
        ));
        Ok(())
    }
}
