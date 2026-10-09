// SPDX-License-Identifier: Apache-2.0
//! Staged edits (docs/05 §7, docs/10 §4): the row key, exact text snapshots
//! (see [`exact`]) and one transaction in which every statement must
//! affect exactly one row. `SET NOCOUNT OFF` keeps the row counts the check
//! needs even when a startup command turned them off.
mod exact;

use std::collections::HashMap;

use openrows_driver_api::{BoxFuture, DriverError, Editor, RowEditInfo, RowSnapshot};
use openrows_sql::select::BoundSql;

use crate::catalog::rows::{ColumnRow, IndexRow};
use crate::ddl::definition::{Relation, quote};
use crate::session::SqlServerSession;
use exact::{exact_guards, exact_text, target_of};

/// SQLSTATE `serialization_failure`: a guarded row changed or vanished.
const CONFLICT: &str = "40001";
const BEGIN: &str = "SET NOCOUNT OFF; BEGIN TRANSACTION";
const ROLLBACK: &str = "IF @@TRANCOUNT > 0 ROLLBACK TRANSACTION";

/// The key: the primary key, else a unique index without a filter over
/// `NOT NULL` columns.
fn row_key(columns: &[ColumnRow], indexes: &[IndexRow]) -> Vec<String> {
    let not_null = |name: &String| {
        columns
            .iter()
            .any(|column| &column.name == name && !column.nullable)
    };
    indexes
        .iter()
        .find(|index| index.primary)
        .or_else(|| {
            indexes.iter().find(|index| {
                index.unique && index.filter.is_none() && index.names().iter().all(not_null)
            })
        })
        .map(IndexRow::names)
        .unwrap_or_default()
}

fn conflict() -> DriverError {
    DriverError::Server {
        code: CONFLICT.to_owned(),
        message: "A row changed, was deleted, or matched more than once. All changes were \
                  rolled back; reload and review."
            .to_owned(),
    }
}

fn uncertain_commit(error: &DriverError) -> DriverError {
    DriverError::Server {
        code: "08007".into(),
        message: format!(
            "Commit outcome is unknown because its acknowledgement was lost: {error}. Reload \
             the table before retrying; the changes may already be committed."
        ),
    }
}

impl SqlServerSession {
    async fn edit_info(&mut self, schema: &str, table: &str) -> Result<RowEditInfo, DriverError> {
        let columns = self.column_rows(schema, table).await?;
        let indexes = self.index_rows(schema, table).await?;
        Ok(RowEditInfo {
            key: row_key(&columns, &indexes),
            protected: columns
                .iter()
                .filter(|column| column.generated())
                .map(|column| column.name.clone())
                .collect(),
            row_id: None,
        })
    }

    async fn snapshot(
        &mut self,
        (schema, table): (&str, &str),
        keys: &[(String, String)],
    ) -> Result<RowSnapshot, DriverError> {
        if keys.is_empty() {
            return Err(DriverError::Unsupported("A stable key is required".into()));
        }
        let columns = self.column_rows(schema, table).await?;
        let projection = columns
            .iter()
            .map(|column| exact_text(&quote(&column.name), &column.base_type))
            .collect::<Vec<_>>()
            .join(", ");
        let predicates = keys
            .iter()
            .enumerate()
            .map(|(index, (name, _))| format!("{} = @P{}", quote(name), index + 1))
            .collect::<Vec<_>>()
            .join(" AND ");
        let relation = Relation { schema, table }.quoted();
        let sql = format!("SELECT TOP (2) {projection} FROM {relation} WHERE {predicates}");
        let params: Vec<Option<String>> =
            keys.iter().map(|(_, value)| Some(value.clone())).collect();
        let mut rows = self.text_query(&sql, &params).await?;
        if rows.len() != 1 {
            return Err(DriverError::Unsupported(
                "The row changed or no longer has a unique identity; reload it".into(),
            ));
        }
        let names = columns.into_iter().map(|column| column.name);
        Ok(names.zip(rows.remove(0)).collect())
    }

    /// The statements with exact guards for their target tables.
    async fn exact(&mut self, statements: &[BoundSql]) -> Result<Vec<BoundSql>, DriverError> {
        let mut types: HashMap<(String, String), Vec<(String, String)>> = HashMap::new();
        let mut out = Vec::with_capacity(statements.len());
        for statement in statements {
            let Some(target) = target_of(&statement.sql) else {
                out.push(statement.clone());
                continue;
            };
            if !types.contains_key(&target) {
                let columns = self.column_rows(&target.0, &target.1).await?;
                let pairs = columns.into_iter().map(|c| (c.name, c.base_type)).collect();
                types.insert(target.clone(), pairs);
            }
            let columns = types.get(&target).map_or(&[][..], Vec::as_slice);
            out.push(BoundSql {
                sql: exact_guards(&statement.sql, columns),
                params: statement.params.clone(),
            });
        }
        Ok(out)
    }

    /// Run `statements` in one transaction; roll back on any failure.
    async fn transaction(
        &mut self,
        statements: &[BoundSql],
        one_row_each: bool,
    ) -> Result<(), DriverError> {
        self.control(BEGIN, &[])
            .await
            .map_err(DriverError::lost_before_commit)?;
        if let Err(error) = self.apply(statements, one_row_each).await {
            // The server rolls back a transaction whose connection ended.
            if error.is_connection_lost() {
                return Err(error.lost_before_commit());
            }
            self.control(ROLLBACK, &[])
                .await
                .map_err(DriverError::lost_before_commit)?;
            return Err(error);
        }
        match self.control("COMMIT TRANSACTION", &[]).await {
            Ok(_) => Ok(()),
            Err(error @ DriverError::Server { .. }) => {
                self.control(ROLLBACK, &[]).await?;
                Err(error)
            }
            Err(error) => Err(uncertain_commit(&error)),
        }
    }

    async fn apply(
        &mut self,
        statements: &[BoundSql],
        one_row_each: bool,
    ) -> Result<(), DriverError> {
        for statement in statements {
            let outcome = self.control(&statement.sql, &statement.params).await?;
            let affected = outcome.counts.last().map(|count| count.rows);
            if one_row_each && affected != Some(1) {
                return Err(conflict());
            }
        }
        Ok(())
    }
}

impl Editor for SqlServerSession {
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
        Box::pin(async move {
            let exact = self.exact(edits).await?;
            self.transaction(&exact, true).await
        })
    }

    /// SQL Server DDL is transactional: a failing statement undoes the set.
    fn commit_ddl<'a>(
        &'a mut self,
        statements: &'a [String],
    ) -> BoxFuture<'a, Result<(), DriverError>> {
        Box::pin(async move {
            let bound: Vec<BoundSql> = statements
                .iter()
                .map(|sql| BoundSql {
                    sql: sql.clone(),
                    params: Vec::new(),
                })
                .collect();
            self.transaction(&bound, false).await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column(name: &str, nullable: bool) -> ColumnRow {
        ColumnRow {
            name: name.to_owned(),
            nullable,
            ..ColumnRow::default()
        }
    }

    fn index(name: &str, columns: &[&str], (unique, primary): (bool, bool)) -> IndexRow {
        IndexRow {
            name: name.to_owned(),
            unique,
            primary,
            columns: columns.iter().map(|c| ((*c).to_owned(), false)).collect(),
            ..IndexRow::default()
        }
    }

    #[test]
    fn keys_prefer_the_primary_key_then_a_not_null_unique_index() {
        let columns = [
            column("id", false),
            column("code", false),
            column("note", true),
        ];
        let primary = index("pk", &["id"], (true, true));
        let unique = index("uq", &["code"], (true, false));
        let nullable = index("uq_note", &["note"], (true, false));
        assert_eq!(row_key(&columns, &[unique.clone(), primary]), ["id"]);
        assert_eq!(row_key(&columns, &[nullable.clone(), unique]), ["code"]);
        assert!(row_key(&columns, &[nullable]).is_empty());
        let error = uncertain_commit(&DriverError::Network(std::io::ErrorKind::BrokenPipe.into()));
        assert!(matches!(error, DriverError::Server { code, .. } if code == "08007"));
    }
}
