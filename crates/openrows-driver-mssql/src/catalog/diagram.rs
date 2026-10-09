// SPDX-License-Identifier: Apache-2.0
//! The ER diagram of one schema in two catalog queries: the columns of its
//! first tables, then every foreign key declared in it (FR-SCHEMA-06).
use openrows_driver_api::DriverError;
use openrows_driver_api::diagram::{
    ColumnRow, DiagramColumn, ForeignKeyRow, SchemaDiagram, assemble,
};

use super::rows::{bind, flag, format_type, optional, text};
use crate::session::SqlServerSession;

/// `{limit}` is replaced with a number.
const COLUMNS_SQL: &str = include_str!("sql/diagram_columns.sql");
const KEYS_SQL: &str = include_str!("sql/diagram_keys.sql");

/// Tables (at most `max_tables`, by name), columns and foreign keys of `schema`.
pub(crate) async fn schema_diagram(
    session: &mut SqlServerSession,
    schema: &str,
    max_tables: usize,
) -> Result<SchemaDiagram, DriverError> {
    let sql = COLUMNS_SQL.replace("{limit}", &max_tables.saturating_add(1).to_string());
    let columns = session.text_query(&sql, &bind(&[schema])).await?;
    let keys = session.text_query(KEYS_SQL, &bind(&[schema])).await?;
    Ok(assemble(
        schema,
        columns.iter().map(|row| column_row(row)).collect(),
        keys.iter().map(|row| key_row(row)).collect(),
        max_tables,
    ))
}

fn column_row(row: &[Option<String>]) -> ColumnRow {
    ColumnRow {
        table: text(row, 0),
        column: optional(row, 1).map(|name| DiagramColumn {
            name,
            data_type: format_type(row, 2),
            nullable: flag(row, 7),
            primary_key: flag(row, 8),
        }),
    }
}

fn key_row(row: &[Option<String>]) -> ForeignKeyRow {
    ForeignKeyRow {
        table: text(row, 0),
        name: text(row, 1),
        column: text(row, 2),
        target_schema: text(row, 3),
        target_table: text(row, 4),
        target_column: text(row, 5),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(cells: &[Option<&str>]) -> Vec<Option<String>> {
        cells.iter().map(|cell| cell.map(str::to_owned)).collect()
    }

    #[test]
    fn rows_map_to_columns_and_key_pairs() {
        let empty = column_row(&row(&[Some("t"), None]));
        assert_eq!(empty.column, None);
        let id = column_row(&row(&[
            Some("orders"),
            Some("id"),
            Some("int"),
            Some("int"),
            Some("4"),
            Some("10"),
            Some("0"),
            Some("0"),
            Some("1"),
        ]));
        assert_eq!(
            id.column.map(|c| (c.data_type, c.nullable, c.primary_key)),
            Some(("int".to_owned(), false, true))
        );
        let key = key_row(&row(&[
            Some("lines"),
            Some("fk_order"),
            Some("order_id"),
            Some("dbo"),
            Some("orders"),
            Some("id"),
        ]));
        assert_eq!(
            (key.target_schema.as_str(), key.target_table.as_str()),
            ("dbo", "orders")
        );
        assert!(COLUMNS_SQL.contains("TOP ({limit})"));
    }
}
