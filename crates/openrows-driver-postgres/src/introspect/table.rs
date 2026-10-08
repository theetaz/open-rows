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

pub(super) fn relation_sql(
    template: &str,
    schema: &str,
    name: &str,
) -> Result<String, DriverError> {
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
        enum_labels: optional(row, 6)
            .map(|text| enum_labels(&text))
            .transpose()?,
    })
}

/// Decode `3:sad5:happy`: character-length prefixes keep any label text unambiguous.
fn enum_labels(text: &str) -> Result<Vec<String>, DriverError> {
    let mut labels = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        let (count, tail) = rest
            .split_once(':')
            .ok_or_else(|| super::malformed("enum label length is missing"))?;
        let count: usize = count
            .parse()
            .map_err(|_| super::malformed("enum label length is not a number"))?;
        let end = tail
            .char_indices()
            .nth(count)
            .map_or(tail.len(), |(index, _)| index);
        let (label, next) = tail.split_at(end);
        if label.chars().count() != count {
            return Err(super::malformed("enum label is truncated"));
        }
        labels.push(label.to_owned());
        rest = next;
    }
    Ok(labels)
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
        assert!(id.enum_labels.is_none());
        assert!(matches!(
            column(&row(&[Some("x"), None, Some("t"), None, None, Some("f")])),
            Err(DriverError::Protocol(_))
        ));
        Ok(())
    }

    #[test]
    fn enum_labels_keep_separators_and_unicode() -> Result<(), DriverError> {
        let mood = column(&row(&[
            Some("mood"),
            Some("mood"),
            Some("t"),
            None,
            None,
            Some("f"),
            Some("3:sad5:a:b,c2:é!"),
        ]))?;
        assert_eq!(
            mood.enum_labels,
            Some(vec!["sad".to_owned(), "a:b,c".to_owned(), "é!".to_owned()])
        );
        assert!(enum_labels("9:short").is_err());
        assert!(enum_labels("x:y").is_err());
        assert!(enum_labels("plain").is_err());
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
