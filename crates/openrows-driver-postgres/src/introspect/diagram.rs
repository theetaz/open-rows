// SPDX-License-Identifier: Apache-2.0
//! The ER diagram of one schema in two catalog queries: the columns of its
//! first tables, then every foreign key declared in it (FR-SCHEMA-06).
use super::{flag, literal, optional, required};
use crate::session::PgSession;
use crate::simple::TextRow;
use openrows_driver_api::DriverError;
use openrows_driver_api::diagram::{
    ColumnRow, DiagramColumn, ForeignKeyRow, SchemaDiagram, assemble,
};

const COLUMNS_SQL: &str = include_str!("sql/diagram-columns.sql");
const KEYS_SQL: &str = include_str!("sql/diagram-keys.sql");

impl PgSession {
    /// Tables (at most `max_tables`, by name), columns and foreign keys of
    /// `schema`.
    ///
    /// # Errors
    /// `Unsupported` for a name containing NUL; transport, protocol or server
    /// failures; a malformed row is `Protocol`.
    pub async fn schema_diagram(
        &mut self,
        schema: &str,
        max_tables: usize,
    ) -> Result<SchemaDiagram, DriverError> {
        let quoted = literal(schema)?;
        let columns_sql = COLUMNS_SQL
            .replace("{schema}", &quoted)
            .replace("{limit}", &max_tables.saturating_add(1).to_string());
        let columns = self.text_query(&columns_sql).await?;
        let keys = self
            .text_query(&KEYS_SQL.replace("{schema}", &quoted))
            .await?;
        Ok(assemble(
            schema,
            columns.iter().map(column_row).collect::<Result<_, _>>()?,
            keys.iter().map(key_row).collect::<Result<_, _>>()?,
            max_tables,
        ))
    }
}

fn column_row(row: &TextRow) -> Result<ColumnRow, DriverError> {
    let column = match optional(row, 1) {
        None => None,
        Some(name) => Some(DiagramColumn {
            name,
            data_type: required(row, 2)?,
            nullable: flag(row, 3)?,
            primary_key: flag(row, 4)?,
        }),
    };
    Ok(ColumnRow {
        table: required(row, 0)?,
        column,
    })
}

fn key_row(row: &TextRow) -> Result<ForeignKeyRow, DriverError> {
    Ok(ForeignKeyRow {
        table: required(row, 0)?,
        name: required(row, 1)?,
        column: required(row, 2)?,
        target_schema: required(row, 3)?,
        target_table: required(row, 4)?,
        target_column: required(row, 5)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(cells: &[Option<&str>]) -> TextRow {
        cells.iter().map(|cell| cell.map(str::to_owned)).collect()
    }

    #[test]
    fn rows_map_to_columns_and_key_pairs() -> Result<(), DriverError> {
        let empty = column_row(&row(&[Some("t"), None, None, None, Some("f")]))?;
        assert_eq!(empty.column, None);
        let id = column_row(&row(&[
            Some("orders"),
            Some("id"),
            Some("bigint"),
            Some("f"),
            Some("t"),
        ]))?;
        assert_eq!(
            id.column.map(|c| (c.nullable, c.primary_key)),
            Some((false, true))
        );
        let key = key_row(&row(&[
            Some("lines"),
            Some("lines_order_fk"),
            Some("order_id"),
            Some("public"),
            Some("orders"),
            Some("id"),
        ]))?;
        assert_eq!(
            (key.target_table.as_str(), key.target_column.as_str()),
            ("orders", "id")
        );
        assert!(matches!(
            key_row(&row(&[Some("x")])),
            Err(DriverError::Protocol(_))
        ));
        Ok(())
    }

    #[test]
    fn templates_have_no_placeholders_left() -> Result<(), DriverError> {
        let sql = COLUMNS_SQL
            .replace("{schema}", &literal("sales")?)
            .replace("{limit}", "501");
        assert!(sql.contains("n.nspname = 'sales'") && sql.contains("LIMIT 501"));
        assert!(!sql.contains('{') && !KEYS_SQL.replace("{schema}", "'s'").contains('{'));
        Ok(())
    }
}
