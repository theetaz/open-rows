// SPDX-License-Identifier: Apache-2.0
//! Catalog listing for the schema tree (docs/05 §6, docs/10 §1), D-06 slice.
//!
//! [`table`] adds a relation's columns, indexes and estimate (D-08).
//! The SQL lives in `sql/*.sql`. Values reach the server as quoted literals
//! because these run through the simple-query protocol (no bind parameters),
//! which keeps each listing to one round trip. Run these on a dedicated metadata
//! session so they never queue behind user queries.
use crate::session::PgSession;
use crate::simple::TextRow;
use openrows_driver_api::DriverError;
use openrows_driver_api::catalog::{DatabaseSummary, ObjectKind, ObjectSummary, SchemaSummary};

mod detail;
mod edit;
mod table;
pub use edit::RowEditInfo;

const DATABASES_SQL: &str = include_str!("sql/databases.sql");
const SCHEMAS_SQL: &str = include_str!("sql/schemas.sql");
const OBJECTS_SQL: &str = include_str!("sql/objects.sql");
const SEARCH_SQL: &str = include_str!("sql/search.sql");

impl PgSession {
    /// Connectable, non-template databases on the server.
    ///
    /// # Errors
    /// Transport, protocol or server failures; a malformed row is `Protocol`.
    pub async fn list_databases(&mut self) -> Result<Vec<DatabaseSummary>, DriverError> {
        let rows = self.text_query(DATABASES_SQL).await?;
        rows.iter()
            .map(|row| {
                Ok(DatabaseSummary {
                    name: required(row, 0)?,
                    system: flag(row, 1)?,
                    comment: optional(row, 2),
                })
            })
            .collect()
    }

    /// Schemas of the connected database, user schemas first.
    ///
    /// # Errors
    /// As [`PgSession::list_databases`].
    pub async fn list_schemas(&mut self) -> Result<Vec<SchemaSummary>, DriverError> {
        let rows = self.text_query(SCHEMAS_SQL).await?;
        rows.iter()
            .map(|row| {
                Ok(SchemaSummary {
                    name: required(row, 0)?,
                    system: flag(row, 1)?,
                    comment: optional(row, 2),
                })
            })
            .collect()
    }

    /// Objects of `kinds` in `schema`, ordered by name. An empty `kinds` lists nothing.
    ///
    /// # Errors
    /// `Unsupported` for a name containing NUL; otherwise as [`PgSession::list_databases`].
    pub async fn list_objects(
        &mut self,
        schema: &str,
        kinds: &[ObjectKind],
    ) -> Result<Vec<ObjectSummary>, DriverError> {
        if kinds.is_empty() {
            return Ok(Vec::new());
        }
        let sql = objects_sql(schema, kinds)?;
        let rows = self.text_query(&sql).await?;
        rows.iter().map(object).collect()
    }

    /// Names of every table, view, sequence and routine outside system
    /// schemas (at most 50 000, the M1-16 budget), for quick open and autocomplete.
    ///
    /// # Errors
    /// As [`PgSession::list_databases`].
    pub async fn search_index(&mut self) -> Result<Vec<ObjectSummary>, DriverError> {
        let rows = self.text_query(SEARCH_SQL).await?;
        rows.iter().map(object).collect()
    }
}

fn objects_sql(schema: &str, kinds: &[ObjectKind]) -> Result<String, DriverError> {
    let kinds = kinds
        .iter()
        .map(|kind| literal(kind.as_str()))
        .collect::<Result<Vec<_>, _>>()?
        .join(", ");
    Ok(OBJECTS_SQL
        .replace("{schema}", &literal(schema)?)
        .replace("{kinds}", &kinds))
}

/// libpq `PQescapeLiteral` rules: double quotes, and use `E''` with doubled
/// backslashes when the text has any, so the result is safe whatever
/// `standard_conforming_strings` says.
pub(crate) fn literal(text: &str) -> Result<String, DriverError> {
    if text.contains('\0') {
        return Err(DriverError::Unsupported(
            "names containing NUL characters".to_owned(),
        ));
    }
    let quoted = text.replace('\'', "''");
    Ok(if quoted.contains('\\') {
        format!("E'{}'", quoted.replace('\\', "\\\\"))
    } else {
        format!("'{quoted}'")
    })
}

/// A double-quoted identifier (embedded quotes doubled); empty names and NUL
/// characters are refused.
pub(crate) fn identifier(name: &str) -> Result<String, DriverError> {
    if name.is_empty() || name.contains('\0') {
        return Err(DriverError::Unsupported("Invalid identifier".into()));
    }
    Ok(format!("\"{}\"", name.replace('"', "\"\"")))
}

fn object(row: &TextRow) -> Result<ObjectSummary, DriverError> {
    let kind = required(row, 2)?;
    Ok(ObjectSummary {
        schema: required(row, 0)?,
        name: required(row, 1)?,
        kind: ObjectKind::parse(&kind)
            .ok_or_else(|| malformed(&format!("unknown object kind {kind}")))?,
        arguments: optional(row, 3),
        row_estimate: optional(row, 4)
            .map(|text| text.parse::<u64>())
            .transpose()
            .map_err(|_| malformed("row estimate is not a number"))?,
        comment: optional(row, 5),
    })
}

fn optional(row: &TextRow, index: usize) -> Option<String> {
    row.get(index).cloned().flatten()
}

fn required(row: &TextRow, index: usize) -> Result<String, DriverError> {
    optional(row, index).ok_or_else(|| malformed(&format!("column {index} is missing")))
}

fn flag(row: &TextRow, index: usize) -> Result<bool, DriverError> {
    match required(row, index)?.as_str() {
        "t" => Ok(true),
        "f" => Ok(false),
        other => Err(malformed(&format!("{other} is not a boolean"))),
    }
}

fn malformed(reason: &str) -> DriverError {
    DriverError::Protocol(format!("unexpected catalog row: {reason}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(cells: &[Option<&str>]) -> TextRow {
        cells.iter().map(|cell| cell.map(str::to_owned)).collect()
    }

    #[test]
    fn literals_are_quoted_like_libpq() -> Result<(), DriverError> {
        assert_eq!(literal("public")?, "'public'");
        assert_eq!(literal("it's")?, "'it''s'");
        assert_eq!(literal(r"a\b'")?, r"E'a\\b'''");
        assert!(matches!(literal("a\0"), Err(DriverError::Unsupported(_))));
        Ok(())
    }

    #[test]
    fn object_sql_fills_schema_and_kinds() -> Result<(), DriverError> {
        let sql = objects_sql("sales", &[ObjectKind::Table, ObjectKind::ForeignTable])?;
        assert!(sql.contains("n.nspname = 'sales'"));
        assert!(sql.contains("o.kind IN ('table', 'foreign_table')"));
        assert!(!sql.contains('{'));
        Ok(())
    }

    #[test]
    fn object_rows_map_to_summaries() -> Result<(), DriverError> {
        let table = object(&row(&[
            Some("public"),
            Some("orders"),
            Some("table"),
            None,
            Some("2400000"),
            Some("Customer orders"),
        ]))?;
        assert_eq!(table.kind, ObjectKind::Table);
        assert_eq!(table.row_estimate, Some(2_400_000));
        assert_eq!(table.comment.as_deref(), Some("Customer orders"));
        let function = object(&row(&[
            Some("public"),
            Some("calc_tax"),
            Some("function"),
            Some("total numeric"),
            None,
            None,
        ]))?;
        assert_eq!(
            (function.kind, function.arguments.as_deref()),
            (ObjectKind::Function, Some("total numeric"))
        );
        Ok(())
    }

    #[test]
    fn malformed_rows_are_protocol_errors() {
        let unknown = object(&row(&[
            Some("s"),
            Some("n"),
            Some("index"),
            None,
            None,
            None,
        ]));
        assert!(matches!(unknown, Err(DriverError::Protocol(_))));
        let estimate = object(&row(&[
            Some("s"),
            Some("n"),
            Some("table"),
            None,
            Some("-1"),
            None,
        ]));
        assert!(matches!(estimate, Err(DriverError::Protocol(_))));
        assert!(matches!(
            flag(&row(&[Some("yes")]), 0),
            Err(DriverError::Protocol(_))
        ));
        assert!(matches!(
            required(&row(&[None]), 0),
            Err(DriverError::Protocol(_))
        ));
    }
}
