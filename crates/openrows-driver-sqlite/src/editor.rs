// SPDX-License-Identifier: Apache-2.0
//! Staged edits on a database file (docs/05 §7, docs/10 §4): the row key,
//! exact text snapshots (`CAST(… AS TEXT)`, the text the commit guards
//! compare) and atomic commits in one `BEGIN IMMEDIATE` transaction.
use openrows_driver_api::{BoxFuture, DriverError, Editor, RowEditInfo, RowSnapshot};
use openrows_sql::Dialect;
use openrows_sql::select::BoundSql;
use rusqlite::{Connection, TransactionBehavior, params_from_iter};

use crate::catalog::detail::indexes;
use crate::catalog::{ColumnRow, column_rows, invalid, sql};
use crate::session::SqliteSession;

/// SQLSTATE `serialization_failure`: a guarded row changed or vanished.
const CONFLICT: &str = "40001";

fn edit_info(
    connection: &Connection,
    schema: &str,
    table: &str,
) -> Result<RowEditInfo, DriverError> {
    let rows = column_rows(connection, schema, table)?;
    let mut keyed: Vec<&ColumnRow> = rows.iter().filter(|row| row.primary_key > 0).collect();
    keyed.sort_by_key(|row| row.primary_key);
    let mut key: Vec<String> = keyed.iter().map(|row| row.name.clone()).collect();
    if key.is_empty() {
        key = unique_not_null(connection, (schema, table), &rows)?;
    }
    Ok(RowEditInfo {
        key,
        protected: rows
            .iter()
            .filter(|row| row.hidden >= 2)
            .map(|row| row.name.clone())
            .collect(),
    })
}

/// Columns of the first unique index whose columns are all `NOT NULL`.
fn unique_not_null(
    connection: &Connection,
    (schema, table): (&str, &str),
    rows: &[ColumnRow],
) -> Result<Vec<String>, DriverError> {
    let not_null = |name: &str| rows.iter().any(|row| row.name == name && row.not_null);
    Ok(indexes(connection, schema, table)?
        .into_iter()
        .filter(|index| index.unique && index.predicate.is_none())
        .map(|index| {
            index
                .columns
                .split(", ")
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .find(|columns| columns.iter().all(|name| not_null(name)))
        .unwrap_or_default())
}

fn snapshot(
    connection: &Connection,
    (schema, table): (&str, &str),
    keys: &[(String, String)],
) -> Result<RowSnapshot, DriverError> {
    if keys.is_empty() {
        return Err(DriverError::Unsupported("A stable key is required".into()));
    }
    let dialect = Dialect::Sqlite;
    let names: Vec<String> = column_rows(connection, schema, table)?
        .into_iter()
        .map(|row| row.name)
        .collect();
    let projection = names
        .iter()
        .map(|name| {
            dialect
                .quote_ident(name)
                .map(|quoted| dialect.text_of(&quoted))
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| invalid(&error))?
        .join(", ");
    let predicates = keys
        .iter()
        .enumerate()
        .map(|(index, (name, _))| {
            dialect
                .quote_ident(name)
                .map(|quoted| format!("{quoted} = ?{}", index + 1))
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| invalid(&error))?
        .join(" AND ");
    let relation = dialect
        .qualified(schema, table)
        .map_err(|error| invalid(&error))?;
    let query = format!("SELECT {projection} FROM {relation} WHERE {predicates} LIMIT 2");
    let mut statement = sql(connection.prepare(&query))?;
    let values = params_from_iter(keys.iter().map(|(_, value)| value.as_str()));
    let rows = sql(statement.query_map(values, |row| {
        (0..names.len())
            .map(|index| row.get::<_, Option<String>>(index))
            .collect::<Result<Vec<_>, _>>()
    }))?;
    let mut rows: Vec<Vec<Option<String>>> = rows.map(sql).collect::<Result<_, _>>()?;
    if rows.len() != 1 {
        return Err(DriverError::Unsupported(
            "The row changed or no longer has a unique identity; reload it".into(),
        ));
    }
    Ok(names.into_iter().zip(rows.remove(0)).collect())
}

/// Run `statements` in one immediate transaction; each `(sql, params)` must
/// change exactly one row when `one_row` is set.
fn commit(
    connection: &mut Connection,
    statements: &[BoundSql],
    one_row: bool,
) -> Result<(), DriverError> {
    let transaction = sql(connection.transaction_with_behavior(TransactionBehavior::Immediate))?;
    for statement in statements {
        let mut prepared = sql(transaction.prepare(&statement.sql))?;
        for (index, value) in statement.params.iter().enumerate() {
            sql(prepared.raw_bind_parameter(index + 1, value.as_deref()))?;
        }
        let changed = sql(prepared.raw_execute())?;
        if one_row && changed != 1 {
            // Dropping the transaction rolls every statement back.
            return Err(DriverError::Server {
                code: CONFLICT.to_owned(),
                message: "A row changed, was deleted, or matched more than once. All changes \
                          were rolled back; reload and review."
                    .to_owned(),
            });
        }
    }
    sql(transaction.commit())
}

impl Editor for SqliteSession {
    fn row_edit_info<'a>(
        &'a mut self,
        schema: &'a str,
        table: &'a str,
    ) -> BoxFuture<'a, Result<RowEditInfo, DriverError>> {
        let (schema, table) = (schema.to_owned(), table.to_owned());
        Box::pin(async move {
            self.worker
                .call(move |connection| edit_info(connection, &schema, &table))
                .await?
        })
    }

    fn edit_snapshot<'a>(
        &'a mut self,
        target: (&'a str, &'a str),
        keys: &'a [(String, String)],
    ) -> BoxFuture<'a, Result<RowSnapshot, DriverError>> {
        let (schema, table, keys) = (target.0.to_owned(), target.1.to_owned(), keys.to_vec());
        Box::pin(async move {
            self.worker
                .call(move |connection| snapshot(connection, (&schema, &table), &keys))
                .await?
        })
    }

    fn commit_edits<'a>(
        &'a mut self,
        edits: &'a [BoundSql],
    ) -> BoxFuture<'a, Result<(), DriverError>> {
        let edits = edits.to_vec();
        Box::pin(async move {
            self.worker
                .call(move |connection| commit(connection, &edits, true))
                .await?
        })
    }

    fn commit_ddl<'a>(
        &'a mut self,
        statements: &'a [String],
    ) -> BoxFuture<'a, Result<(), DriverError>> {
        let statements: Vec<BoundSql> = statements
            .iter()
            .map(|sql| BoundSql {
                sql: sql.clone(),
                params: Vec::new(),
            })
            .collect();
        Box::pin(async move {
            self.worker
                .call(move |connection| commit(connection, &statements, false))
                .await?
        })
    }
}
