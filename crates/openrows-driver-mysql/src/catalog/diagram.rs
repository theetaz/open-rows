// SPDX-License-Identifier: Apache-2.0
//! The ER diagram of one database in two `information_schema` queries: the
//! columns of its first tables, then every foreign key declared in it
//! (FR-SCHEMA-06).
use openrows_driver_api::DriverError;
use openrows_driver_api::diagram::{
    ColumnRow, DiagramColumn, ForeignKeyRow, SchemaDiagram, assemble,
};

use super::{bind, optional, text};
use crate::session::MySqlSession;

/// `{limit}` is replaced with a number; the schema is bound twice.
const COLUMNS_SQL: &str = "SELECT t.TABLE_NAME, c.COLUMN_NAME, c.COLUMN_TYPE, c.IS_NULLABLE, \
    c.COLUMN_KEY FROM (SELECT TABLE_NAME FROM information_schema.TABLES \
    WHERE TABLE_SCHEMA = ? AND TABLE_TYPE NOT IN ('VIEW', 'SYSTEM VIEW', 'SEQUENCE') \
    ORDER BY TABLE_NAME LIMIT {limit}) AS t \
    LEFT JOIN information_schema.COLUMNS AS c \
    ON c.TABLE_SCHEMA = ? AND c.TABLE_NAME = t.TABLE_NAME \
    ORDER BY t.TABLE_NAME, c.ORDINAL_POSITION";
const KEYS_SQL: &str = "SELECT TABLE_NAME, CONSTRAINT_NAME, COLUMN_NAME, \
    REFERENCED_TABLE_SCHEMA, REFERENCED_TABLE_NAME, REFERENCED_COLUMN_NAME \
    FROM information_schema.KEY_COLUMN_USAGE \
    WHERE TABLE_SCHEMA = ? AND REFERENCED_TABLE_NAME IS NOT NULL \
    ORDER BY TABLE_NAME, CONSTRAINT_NAME, ORDINAL_POSITION";

/// Tables (at most `max_tables`, by name), columns and foreign keys of `schema`.
pub(crate) async fn schema_diagram(
    session: &mut MySqlSession,
    schema: &str,
    max_tables: usize,
) -> Result<SchemaDiagram, DriverError> {
    let sql = COLUMNS_SQL.replace("{limit}", &max_tables.saturating_add(1).to_string());
    let columns = session.text_query(&sql, &bind(&[schema, schema])).await?;
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
            data_type: text(row, 2),
            nullable: text(row, 3) == "YES",
            primary_key: text(row, 4) == "PRI",
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
        let empty = column_row(&row(&[Some("t"), None, None, None, None]));
        assert_eq!(empty.column, None);
        let id = column_row(&row(&[
            Some("orders"),
            Some("id"),
            Some("int unsigned"),
            Some("NO"),
            Some("PRI"),
        ]));
        assert_eq!(
            id.column.map(|c| (c.data_type, c.nullable, c.primary_key)),
            Some(("int unsigned".to_owned(), false, true))
        );
        let key = key_row(&row(&[
            Some("lines"),
            Some("fk_order"),
            Some("order_id"),
            Some("shop"),
            Some("orders"),
            Some("id"),
        ]));
        assert_eq!(
            (key.target_schema.as_str(), key.target_table.as_str()),
            ("shop", "orders")
        );
        assert!(COLUMNS_SQL.contains("LIMIT {limit}"));
    }
}
