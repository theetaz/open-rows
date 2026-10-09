// SPDX-License-Identifier: Apache-2.0
//! Resolving an export's source into what the job reads: a stored result
//! (or some of its rows), or statements for a session of its own.
use std::sync::Arc;

use arrow_schema::SchemaRef;
use openrows_core::export::ExportPart;
use openrows_results::{ResultStore, StoredBatches};
use openrows_sql::select::{BoundSql, TableRef, all_rows_sql};
use openrows_sql::{Dialect, StatementKind, classify, placeholders, split};
use tauri::{AppHandle, Manager};

use super::dto::{ExportSourceDto, SchemaTarget};
use crate::connections::catalog::read_columns;
use crate::connections::{Connections, IpcError, IpcErrorKind};
use crate::jobs::JobScope as Scope;
use crate::tables::commands::{ensure_read, owned_session};

/// Most tables one export may hold.
const MAX_TABLES: usize = 10_000;

/// What an export reads.
pub(super) enum Plan {
    /// A stored result, optionally only the rows at these positions.
    Stored {
        result_id: String,
        schema: SchemaRef,
        batches: StoredBatches,
        rows: Option<Vec<u32>>,
    },
    /// Statements run on a session of the job's own into one file.
    Statements(Vec<ExportPart>),
    /// One file per table, named after it, into a folder.
    PerTable(Vec<(String, BoundSql)>),
}

/// Resolve `source`; `workbook` puts several tables into one file.
pub(super) async fn resolve(
    app: &AppHandle,
    scope: &Scope,
    source: &ExportSourceDto,
) -> Result<Plan, IpcError> {
    let single = |statement| {
        Plan::Statements(vec![ExportPart {
            table: None,
            statement,
        }])
    };
    match source {
        ExportSourceDto::Result { result_id, .. } => stored(app, result_id, None),
        ExportSourceDto::Rows {
            result_id, rows, ..
        } => stored(app, result_id, Some(rows.clone())),
        ExportSourceDto::Query { sql, .. } => Ok(single(read_statement(sql, scope.dialect())?)),
        ExportSourceDto::Table { session_id, target } => {
            let connections = app.state::<Connections>();
            owned_session(&connections, session_id, target)?;
            let name = (target.schema.as_str(), target.table.as_str());
            Ok(single(table_rows(&connections, scope, name).await?))
        }
        ExportSourceDto::Tables {
            target,
            tables,
            workbook,
            ..
        } => tables_plan(app, scope, (target, tables), *workbook).await,
    }
}

fn stored(app: &AppHandle, result_id: &str, rows: Option<Vec<u32>>) -> Result<Plan, IpcError> {
    let (schema, batches) = app
        .state::<Arc<ResultStore>>()
        .batches(result_id)
        .ok_or_else(|| {
            IpcError::new(
                IpcErrorKind::NotFound,
                "This result is no longer available; run the query again",
            )
        })?;
    if rows.as_ref().is_some_and(Vec::is_empty) {
        return Err(IpcError::invalid("Select the rows to export"));
    }
    Ok(Plan::Stored {
        result_id: result_id.to_owned(),
        schema,
        batches,
        rows,
    })
}

async fn tables_plan(
    app: &AppHandle,
    scope: &Scope,
    (target, tables): (&SchemaTarget, &[String]),
    workbook: bool,
) -> Result<Plan, IpcError> {
    if target.profile_id != scope.profile_id || target.database != scope.database {
        return Err(IpcError::invalid("The session runs on another database"));
    }
    if tables.is_empty() || tables.len() > MAX_TABLES {
        return Err(IpcError::invalid("Choose the tables to export"));
    }
    let connections = app.state::<Connections>();
    let mut statements = Vec::with_capacity(tables.len());
    for table in tables {
        let name = (target.schema.as_str(), table.as_str());
        let statement = table_rows(&connections, scope, name).await?;
        statements.push((table.clone(), statement));
    }
    Ok(if workbook {
        Plan::Statements(
            statements
                .into_iter()
                .map(|(table, statement)| ExportPart {
                    table: Some(table),
                    statement,
                })
                .collect(),
        )
    } else {
        Plan::PerTable(statements)
    })
}

/// `SELECT *` of an existing table or view.
async fn table_rows(
    connections: &Connections,
    scope: &Scope,
    (schema, table): (&str, &str),
) -> Result<BoundSql, IpcError> {
    let columns = read_columns(
        connections,
        (&scope.profile_id, &scope.database),
        (schema.to_owned(), table.to_owned()),
    )
    .await?;
    let known: Vec<String> = columns.into_iter().map(|column| column.name).collect();
    let relation = TableRef {
        schema: schema.to_owned(),
        name: table.to_owned(),
    };
    let bound = all_rows_sql(scope.dialect(), &relation, &known).map_err(|_| {
        IpcError::new(
            IpcErrorKind::NotFound,
            format!("{schema}.{table} was not found"),
        )
    })?;
    ensure_read(&bound, scope.dialect())?;
    Ok(bound)
}

/// One plain read statement to run again; anything else is refused.
pub(super) fn read_statement(sql: &str, dialect: Dialect) -> Result<BoundSql, IpcError> {
    let refused =
        || IpcError::invalid("Only a single read-only statement can be run again for export");
    let statements = split(sql, dialect);
    let [statement] = statements.as_slice() else {
        return Err(refused());
    };
    let text = statement.text(sql);
    let class = classify(text, dialect);
    if class.kind != StatementKind::Read || class.dangerous {
        return Err(refused());
    }
    // Bound values exist only in the run request; export the received rows instead.
    if !placeholders(text, dialect).is_empty() {
        return Err(IpcError::invalid(
            "A statement with parameters cannot be run again for export; export the received rows",
        ));
    }
    Ok(BoundSql {
        sql: text.to_owned(),
        params: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_one_plain_read_can_be_run_again() -> Result<(), IpcError> {
        let bound = read_statement("  SELECT * FROM t ;  ", Dialect::Postgres)?;
        assert_eq!(bound.sql, "SELECT * FROM t");
        assert!(bound.params.is_empty());
        for refused in [
            "DELETE FROM t",
            "SELECT 1; SELECT 2",
            "UPDATE t SET a = 1",
            "",
            "WITH d AS (DELETE FROM t RETURNING *) SELECT * FROM d",
            "SELECT * FROM t WHERE id = ?",
        ] {
            assert!(
                read_statement(refused, Dialect::MySql).is_err(),
                "{refused}"
            );
        }
        Ok(())
    }
}
