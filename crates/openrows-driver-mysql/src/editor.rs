// SPDX-License-Identifier: Apache-2.0
//! Staged edits (docs/05 §7, docs/10 §4): the row key, exact text snapshots
//! (`CAST(… AS CHAR)`, what the commit guards compare) and one transaction
//! in which every statement must affect exactly one row. `CLIENT_FOUND_ROWS`
//! makes an `UPDATE` that writes the same values still count its row.
use openrows_driver_api::{BoxFuture, DriverError, Editor, RowEditInfo, RowSnapshot};
use openrows_sql::Dialect;
use openrows_sql::select::BoundSql;

use crate::session::MySqlSession;

/// SQLSTATE `serialization_failure`: a guarded row changed or vanished.
const CONFLICT: &str = "40001";

fn invalid(error: &openrows_sql::select::SelectError) -> DriverError {
    DriverError::Unsupported(error.to_string())
}

impl MySqlSession {
    async fn edit_info(&mut self, schema: &str, table: &str) -> Result<RowEditInfo, DriverError> {
        let columns = self.column_rows(schema, table).await?;
        let indexes = self.index_rows(schema, table).await?;
        let not_null = |name: &String| {
            columns
                .iter()
                .any(|column| &column.name == name && !column.nullable)
        };
        let usable = |index: &&crate::catalog::IndexRow| index.unique && !index.partial;
        let key = indexes
            .iter()
            .filter(usable)
            .find(|index| index.name == "PRIMARY")
            .or_else(|| {
                indexes
                    .iter()
                    .filter(usable)
                    .find(|index| index.columns.iter().all(not_null))
            })
            .map(|index| index.columns.clone())
            .unwrap_or_default();
        Ok(RowEditInfo {
            key,
            protected: columns
                .iter()
                .filter(|column| column.generated())
                .map(|column| column.name.clone())
                .collect(),
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
        let dialect = Dialect::MySql;
        let names: Vec<String> = self
            .column_rows(schema, table)
            .await?
            .into_iter()
            .map(|c| c.name)
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
            .map(|(name, _)| {
                dialect
                    .quote_ident(name)
                    .map(|quoted| format!("{quoted} = ?"))
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| invalid(&error))?
            .join(" AND ");
        let relation = dialect
            .qualified(schema, table)
            .map_err(|error| invalid(&error))?;
        let sql = format!("SELECT {projection} FROM {relation} WHERE {predicates} LIMIT 2");
        let params: Vec<Option<String>> =
            keys.iter().map(|(_, value)| Some(value.clone())).collect();
        let mut rows = self.text_query(&sql, &params).await?;
        if rows.len() != 1 {
            return Err(DriverError::Unsupported(
                "The row changed or no longer has a unique identity; reload it".into(),
            ));
        }
        Ok(names.into_iter().zip(rows.remove(0)).collect())
    }

    /// Commit `statements` in one transaction; roll back on any failure.
    async fn commit(&mut self, statements: &[BoundSql]) -> Result<(), DriverError> {
        self.control("START TRANSACTION", &[]).await?;
        let applied = self.apply(statements).await;
        if let Err(error) = applied {
            self.control("ROLLBACK", &[]).await?;
            return Err(error);
        }
        match self.control("COMMIT", &[]).await {
            Ok(_) => Ok(()),
            Err(error @ DriverError::Server { .. }) => {
                self.control("ROLLBACK", &[]).await?;
                Err(error)
            }
            Err(error) => Err(uncertain_commit(&error)),
        }
    }

    async fn apply(&mut self, statements: &[BoundSql]) -> Result<(), DriverError> {
        for statement in statements {
            let outcome = self.control(&statement.sql, &statement.params).await?;
            if outcome.affected != 1 {
                return Err(DriverError::Server {
                    code: CONFLICT.to_owned(),
                    message: "A row changed, was deleted, or matched more than once. All changes \
                              were rolled back; reload and review."
                        .to_owned(),
                });
            }
        }
        Ok(())
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

impl Editor for MySqlSession {
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
        Box::pin(self.commit(edits))
    }

    /// `MySQL` commits each DDL statement implicitly, so they run one by
    /// one and stop at the first failure (earlier ones stay applied).
    fn commit_ddl<'a>(
        &'a mut self,
        statements: &'a [String],
    ) -> BoxFuture<'a, Result<(), DriverError>> {
        Box::pin(async move {
            for sql in statements {
                self.control(sql, &[]).await?;
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lost_commit_acknowledgement_is_never_reported_as_rolled_back() {
        let error = uncertain_commit(&DriverError::Network(std::io::ErrorKind::BrokenPipe.into()));
        assert!(
            matches!(error, DriverError::Server { code, message } if code == "08007" && message.contains("may already be committed"))
        );
    }
}
