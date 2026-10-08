// SPDX-License-Identifier: Apache-2.0
//! Raw `information_schema` rows shared by the tree, the table detail and
//! the editor: columns and indexes.
use openrows_driver_api::DriverError;

use super::{bind, optional, text};
use crate::session::MySqlSession;

const COLUMNS_SQL: &str = "SELECT COLUMN_NAME, COLUMN_TYPE, IS_NULLABLE, COLUMN_DEFAULT, \
    COLUMN_COMMENT, COLUMN_KEY, EXTRA, GENERATION_EXPRESSION \
    FROM information_schema.COLUMNS WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? \
    ORDER BY ORDINAL_POSITION";
const INDEXES_SQL: &str = "SELECT INDEX_NAME, NON_UNIQUE, COLUMN_NAME, INDEX_TYPE, SUB_PART \
    FROM information_schema.STATISTICS WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? \
    ORDER BY INDEX_NAME, SEQ_IN_INDEX";

/// One `information_schema.COLUMNS` row.
#[derive(Clone, Debug)]
pub(crate) struct ColumnRow {
    pub(crate) name: String,
    pub(crate) column_type: String,
    pub(crate) nullable: bool,
    pub(crate) default: Option<String>,
    pub(crate) comment: Option<String>,
    pub(crate) key: String,
    pub(crate) extra: String,
    pub(crate) generation: Option<String>,
}

impl ColumnRow {
    /// A generated (virtual or stored) column, which edits never write.
    /// `DEFAULT_GENERATED` (an expression default) is not one.
    pub(crate) fn generated(&self) -> bool {
        let extra = self.extra.to_ascii_uppercase();
        ["VIRTUAL", "PERSISTENT", "STORED"]
            .iter()
            .any(|word| extra.split_whitespace().any(|part| part == *word))
    }
}

/// One index, columns in key order.
#[derive(Clone, Debug, Default)]
pub(crate) struct IndexRow {
    pub(crate) name: String,
    pub(crate) unique: bool,
    pub(crate) columns: Vec<String>,
    pub(crate) method: String,
    /// Some column is a prefix (`col(10)`) or an expression.
    pub(crate) partial: bool,
}

/// `enum('a','b')` → `["a", "b"]`.
pub(crate) fn enum_labels(column_type: &str) -> Option<Vec<String>> {
    let inner = column_type.strip_prefix("enum(")?.strip_suffix(')')?;
    let mut labels = Vec::new();
    for quoted in inner.split("','") {
        labels.push(quoted.trim_matches('\'').replace("''", "'"));
    }
    Some(labels)
}

impl MySqlSession {
    pub(crate) async fn column_rows(
        &mut self,
        schema: &str,
        table: &str,
    ) -> Result<Vec<ColumnRow>, DriverError> {
        let rows = self
            .text_query(COLUMNS_SQL, &bind(&[schema, table]))
            .await?;
        Ok(rows
            .iter()
            .map(|row| ColumnRow {
                name: text(row, 0),
                column_type: text(row, 1),
                nullable: text(row, 2) == "YES",
                default: row.get(3).cloned().flatten(),
                comment: optional(row, 4),
                key: text(row, 5),
                extra: text(row, 6),
                generation: optional(row, 7),
            })
            .collect())
    }

    pub(crate) async fn index_rows(
        &mut self,
        schema: &str,
        table: &str,
    ) -> Result<Vec<IndexRow>, DriverError> {
        let rows = self
            .text_query(INDEXES_SQL, &bind(&[schema, table]))
            .await?;
        let mut indexes: Vec<IndexRow> = Vec::new();
        for row in &rows {
            let name = text(row, 0);
            if indexes.last().is_none_or(|index| index.name != name) {
                indexes.push(IndexRow {
                    unique: text(row, 1) == "0",
                    method: text(row, 3).to_ascii_lowercase(),
                    name,
                    ..IndexRow::default()
                });
            }
            if let Some(index) = indexes.last_mut() {
                let column = optional(row, 2);
                index.partial |= column.is_none() || optional(row, 4).is_some();
                index
                    .columns
                    .push(column.unwrap_or_else(|| "<expression>".to_owned()));
            }
        }
        Ok(indexes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enum_labels_unquote_the_column_type() {
        assert_eq!(
            enum_labels("enum('a','it''s','c d')"),
            Some(vec!["a".to_owned(), "it's".to_owned(), "c d".to_owned()])
        );
        assert_eq!(enum_labels("varchar(10)"), None);
    }

    #[test]
    fn generated_columns_are_recognised_on_both_servers() {
        let row = |extra: &str| ColumnRow {
            name: "c".into(),
            column_type: "int".into(),
            nullable: true,
            default: None,
            comment: None,
            key: String::new(),
            extra: extra.into(),
            generation: None,
        };
        assert!(row("VIRTUAL GENERATED").generated());
        assert!(row("STORED GENERATED").generated());
        assert!(row("PERSISTENT").generated());
        assert!(!row("auto_increment").generated());
        assert!(!row("DEFAULT_GENERATED on update CURRENT_TIMESTAMP").generated());
    }
}
