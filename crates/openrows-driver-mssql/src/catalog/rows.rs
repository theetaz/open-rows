// SPDX-License-Identifier: Apache-2.0
//! Raw catalog rows shared by the tree, the table detail, the DDL and the
//! editor: columns (with the declared type formatted as T-SQL) and indexes.
use openrows_driver_api::DriverError;

use crate::session::SqlServerSession;

const COLUMNS_SQL: &str = include_str!("sql/columns.sql");
const INDEXES_SQL: &str = include_str!("sql/indexes.sql");

/// Column `index` of a text row, or empty.
pub(crate) fn text(row: &[Option<String>], index: usize) -> String {
    row.get(index).cloned().flatten().unwrap_or_default()
}

/// Column `index` of a text row; NULL and empty text are `None`.
pub(crate) fn optional(row: &[Option<String>], index: usize) -> Option<String> {
    row.get(index)
        .cloned()
        .flatten()
        .filter(|value| !value.is_empty())
}

/// A `bit` or `0`/`1` column.
pub(crate) fn flag(row: &[Option<String>], index: usize) -> bool {
    text(row, index) == "1"
}

/// Names as bound text parameters.
pub(crate) fn bind(values: &[&str]) -> Vec<Option<String>> {
    values
        .iter()
        .map(|value| Some((*value).to_owned()))
        .collect()
}

/// The declared type of a column, e.g. `nvarchar(40)`, `decimal(10, 2)`,
/// `varbinary(max)` or an alias type's own name (`sysname`).
pub(crate) fn format_type(row: &[Option<String>], first: usize) -> String {
    let (name, base) = (text(row, first), text(row, first + 1));
    if !name.eq_ignore_ascii_case(&base) {
        return name;
    }
    let number = |offset: usize| text(row, first + offset).parse::<i64>().unwrap_or(0);
    let (length, precision, scale) = (number(2), number(3), number(4));
    let size = |bytes: i64| {
        if bytes < 0 {
            "max".to_owned()
        } else {
            bytes.to_string()
        }
    };
    match base.as_str() {
        "char" | "varchar" | "binary" | "varbinary" => format!("{name}({})", size(length)),
        "nchar" | "nvarchar" => {
            format!("{name}({})", size(if length < 0 { -1 } else { length / 2 }))
        }
        "decimal" | "numeric" => format!("{name}({precision}, {scale})"),
        "time" | "datetime2" | "datetimeoffset" if scale != 7 => format!("{name}({scale})"),
        "float" if precision != 53 => format!("{name}({precision})"),
        _ => name,
    }
}

/// One column with everything the catalog, DDL and editor use.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ColumnRow {
    pub(crate) name: String,
    pub(crate) data_type: String,
    /// The system type name, e.g. `nvarchar` for a `sysname` column.
    pub(crate) base_type: String,
    pub(crate) nullable: bool,
    /// Default expression, e.g. `((1))`.
    pub(crate) default: Option<String>,
    /// Name of the default constraint.
    pub(crate) default_name: Option<String>,
    pub(crate) comment: Option<String>,
    pub(crate) primary_key: bool,
    /// Identity seed and increment.
    pub(crate) identity: Option<(String, String)>,
    /// Computed-column expression.
    pub(crate) computed: Option<String>,
    pub(crate) collation: Option<String>,
}

impl ColumnRow {
    /// Server-generated: never written by edits.
    pub(crate) fn generated(&self) -> bool {
        self.identity.is_some() || self.computed.is_some() || self.base_type == "timestamp"
    }

    fn from_row(row: &[Option<String>]) -> Self {
        Self {
            name: text(row, 0),
            data_type: format_type(row, 1),
            base_type: text(row, 2).to_ascii_lowercase(),
            nullable: flag(row, 6),
            default: optional(row, 7),
            default_name: optional(row, 8),
            comment: optional(row, 9),
            primary_key: flag(row, 10),
            identity: flag(row, 11).then(|| (text(row, 14), text(row, 15))),
            computed: optional(row, 12),
            collation: optional(row, 13),
        }
    }
}

/// One index, key columns in key order.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct IndexRow {
    pub(crate) name: String,
    pub(crate) unique: bool,
    pub(crate) primary: bool,
    /// Backs a `UNIQUE` constraint.
    pub(crate) unique_constraint: bool,
    /// e.g. `clustered`, `nonclustered`, `clustered columnstore`.
    pub(crate) method: String,
    pub(crate) filter: Option<String>,
    /// Key columns with `DESC` when descending.
    pub(crate) columns: Vec<(String, bool)>,
    pub(crate) include: Vec<String>,
}

impl IndexRow {
    /// A key or included column from an index row.
    fn add_column(&mut self, row: &[Option<String>]) {
        if flag(row, 7) {
            self.include.push(text(row, 6));
        } else {
            self.columns.push((text(row, 6), flag(row, 8)));
        }
    }

    /// Key column names only.
    pub(crate) fn names(&self) -> Vec<String> {
        self.columns.iter().map(|(name, _)| name.clone()).collect()
    }

    /// `a, b DESC`.
    pub(crate) fn key_list(&self) -> String {
        self.columns
            .iter()
            .map(|(name, descending)| {
                if *descending {
                    format!("{name} DESC")
                } else {
                    name.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl SqlServerSession {
    pub(crate) async fn column_rows(
        &mut self,
        schema: &str,
        table: &str,
    ) -> Result<Vec<ColumnRow>, DriverError> {
        let rows = self
            .text_query(COLUMNS_SQL, &bind(&[schema, table]))
            .await?;
        Ok(rows.iter().map(|row| ColumnRow::from_row(row)).collect())
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
                    unique: flag(row, 1),
                    primary: flag(row, 2),
                    unique_constraint: flag(row, 3),
                    method: text(row, 4),
                    filter: optional(row, 5),
                    name,
                    ..IndexRow::default()
                });
            }
            if let Some(index) = indexes.last_mut() {
                index.add_column(row);
            }
        }
        Ok(indexes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(cells: &[&str]) -> Vec<Option<String>> {
        cells.iter().map(|cell| Some((*cell).to_owned())).collect()
    }

    #[test]
    fn types_print_with_their_lengths() {
        assert_eq!(
            format_type(&row(&["nvarchar", "nvarchar", "80", "0", "0"]), 0),
            "nvarchar(40)"
        );
        assert_eq!(
            format_type(&row(&["varbinary", "varbinary", "-1", "0", "0"]), 0),
            "varbinary(max)"
        );
        assert_eq!(
            format_type(&row(&["decimal", "decimal", "9", "10", "2"]), 0),
            "decimal(10, 2)"
        );
        assert_eq!(
            format_type(&row(&["datetime2", "datetime2", "8", "27", "7"]), 0),
            "datetime2"
        );
        assert_eq!(
            format_type(&row(&["time", "time", "5", "12", "3"]), 0),
            "time(3)"
        );
        assert_eq!(
            format_type(&row(&["sysname", "nvarchar", "256", "0", "0"]), 0),
            "sysname"
        );
        assert_eq!(format_type(&row(&["int", "int", "4", "10", "0"]), 0), "int");
    }
}
