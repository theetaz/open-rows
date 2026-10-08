// SPDX-License-Identifier: Apache-2.0
//! A browsed relation's columns, indexes and row estimate (D-08 table view).
use super::{flag, literal, optional, required};
use crate::session::PgSession;
use crate::simple::TextRow;
use openrows_driver_api::DriverError;
use openrows_driver_api::catalog::{ColumnSummary, IndexSummary, TableStructure};

const COLUMNS_SQL: &str = include_str!("sql/columns.sql");
const INDEXES_SQL: &str = include_str!("sql/indexes.sql");
const ESTIMATE_SQL: &str = include_str!("sql/estimate.sql");

impl PgSession {
    /// Columns of `schema`.`name` in ordinal order; empty when the relation
    /// does not exist (or is not table-like).
    ///
    /// # Errors
    /// As [`PgSession::table_structure`].
    pub async fn table_columns(
        &mut self,
        schema: &str,
        name: &str,
    ) -> Result<Vec<ColumnSummary>, DriverError> {
        let rows = self
            .text_query(&relation_sql(COLUMNS_SQL, schema, name)?)
            .await?;
        rows.iter().map(column).collect()
    }

    /// Columns, indexes and planner row estimate of `schema`.`name`; every
    /// list is empty when the relation does not exist.
    ///
    /// # Errors
    /// `Unsupported` for names containing NUL; transport, protocol or server
    /// failures; a malformed row is `Protocol`.
    pub async fn table_structure(
        &mut self,
        schema: &str,
        name: &str,
    ) -> Result<TableStructure, DriverError> {
        let columns = self.table_columns(schema, name).await?;
        let indexes = self
            .text_query(&relation_sql(INDEXES_SQL, schema, name)?)
            .await?;
        let estimate = self
            .text_query(&relation_sql(ESTIMATE_SQL, schema, name)?)
            .await?;
        Ok(TableStructure {
            columns,
            indexes: indexes.iter().map(index).collect::<Result<_, _>>()?,
            row_estimate: row_estimate(&estimate)?,
        })
    }
}

fn relation_sql(template: &str, schema: &str, name: &str) -> Result<String, DriverError> {
    Ok(template
        .replace("{schema}", &literal(schema)?)
        .replace("{name}", &literal(name)?))
}

fn column(row: &TextRow) -> Result<ColumnSummary, DriverError> {
    Ok(ColumnSummary {
        name: required(row, 0)?,
        data_type: required(row, 1)?,
        nullable: flag(row, 2)?,
        default: optional(row, 3),
        comment: optional(row, 4),
        primary_key: flag(row, 5)?,
    })
}

fn index(row: &TextRow) -> Result<IndexSummary, DriverError> {
    Ok(IndexSummary {
        name: required(row, 0)?,
        columns: optional(row, 1).unwrap_or_default(),
        unique: flag(row, 2)?,
        primary: flag(row, 3)?,
        method: required(row, 4)?,
        predicate: optional(row, 5),
    })
}

fn row_estimate(rows: &[TextRow]) -> Result<Option<u64>, DriverError> {
    rows.first()
        .and_then(|row| optional(row, 0))
        .map(|text| text.parse::<u64>())
        .transpose()
        .map_err(|_| super::malformed("row estimate is not a number"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(cells: &[Option<&str>]) -> TextRow {
        cells.iter().map(|cell| cell.map(str::to_owned)).collect()
    }

    #[test]
    fn relation_sql_quotes_schema_and_name() -> Result<(), DriverError> {
        let sql = relation_sql(COLUMNS_SQL, "sales", "it's")?;
        assert!(sql.contains("n.nspname = 'sales'"));
        assert!(sql.contains("c.relname = 'it''s'"));
        assert!(!sql.contains('{'));
        assert!(matches!(
            relation_sql(INDEXES_SQL, "a\0", "b"),
            Err(DriverError::Unsupported(_))
        ));
        Ok(())
    }

    #[test]
    fn column_rows_map_to_summaries() -> Result<(), DriverError> {
        let id = column(&row(&[
            Some("id"),
            Some("integer"),
            Some("f"),
            Some("GENERATED ALWAYS AS IDENTITY"),
            None,
            Some("t"),
        ]))?;
        assert_eq!(
            (id.name.as_str(), id.nullable, id.default.as_deref()),
            ("id", false, Some("GENERATED ALWAYS AS IDENTITY"))
        );
        assert!(id.primary_key);
        assert!(matches!(
            column(&row(&[Some("x"), None, Some("t"), None, None, Some("f")])),
            Err(DriverError::Protocol(_))
        ));
        Ok(())
    }

    #[test]
    fn index_rows_map_to_summaries() -> Result<(), DriverError> {
        let unique = index(&row(&[
            Some("users_email_key"),
            Some("lower(email), id"),
            Some("t"),
            Some("f"),
            Some("btree"),
            Some("deleted_at IS NULL"),
        ]))?;
        assert_eq!(unique.columns, "lower(email), id");
        assert!(unique.unique && !unique.primary);
        assert_eq!(unique.predicate.as_deref(), Some("deleted_at IS NULL"));
        assert!(matches!(
            index(&row(&[
                Some("i"),
                None,
                Some("maybe"),
                Some("f"),
                Some("gin"),
                None
            ])),
            Err(DriverError::Protocol(_))
        ));
        Ok(())
    }

    #[test]
    fn estimates_are_optional_numbers() -> Result<(), DriverError> {
        assert_eq!(row_estimate(&[row(&[Some("1811")])])?, Some(1811));
        assert_eq!(row_estimate(&[row(&[None])])?, None);
        assert_eq!(row_estimate(&[])?, None);
        assert!(row_estimate(&[row(&[Some("-1")])]).is_err());
        Ok(())
    }
}
