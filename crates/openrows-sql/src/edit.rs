// SPDX-License-Identifier: Apache-2.0
//! Parameterised, optimistic table edits; identifiers come from catalog metadata.
//!
//! Every `UPDATE` and `DELETE` is guarded by the row's complete original
//! snapshot: each column's text form (see [`Dialect::text_of`]) must still
//! equal the snapshot, NULL-safely and byte for byte, so a row changed by
//! someone else matches nothing and the commit rolls back.
use crate::Dialect;
use crate::select::{BoundSql, SelectError, TableRef};
use std::collections::BTreeMap;

/// Exact server text values, never display-formatted or rounded values.
pub type Values = BTreeMap<String, Option<String>>;
/// Staged field values by column name.
pub type Changes = BTreeMap<String, CellValue>;
/// One staged field value: NULL, the column default, or exact server text input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CellValue {
    /// SQL NULL, bound as a NULL parameter.
    Null,
    /// The column default: the `DEFAULT` keyword, never a bound text value.
    Default,
    /// Exact text input, bound as a parameter.
    Text(String),
}
/// The operation to stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditKind {
    /// Change specified fields of an existing row.
    Update,
    /// Create a new row.
    Insert,
    /// Remove an identified row.
    Delete,
}
/// One row mutation with its complete original snapshot.
#[derive(Clone, Debug)]
pub struct Edit {
    /// Operation to perform.
    pub kind: EditKind,
    /// Complete original text snapshot.
    pub original: Values,
    /// New field values.
    pub values: Changes,
}
/// Catalog constraints needed before SQL can be generated.
pub struct EditTable {
    /// Catalog relation identity.
    pub table: TableRef,
    /// All catalog columns.
    pub columns: Vec<String>,
    /// Non-null unique identity columns.
    pub key: Vec<String>,
    /// Server-generated columns excluded from writes.
    pub protected: Vec<String>,
}

/// Reject unknown/protected columns and incomplete snapshots before constructing SQL.
///
/// `DEFAULT` values become the `DEFAULT` keyword in an `UPDATE` and are omitted from an
/// `INSERT` column list (all omitted: `DEFAULT VALUES`, or `() VALUES ()` for `MySQL`);
/// they never become parameters. `SQLite` has no `DEFAULT` in `UPDATE`.
///
/// # Errors
/// Invalid identifiers, NUL data, unknown columns, a missing stable row
/// identity, or a `DEFAULT` the dialect cannot express.
pub fn edit_sql(dialect: Dialect, table: &EditTable, edit: &Edit) -> Result<BoundSql, SelectError> {
    if table.key.is_empty() || table.columns.is_empty() {
        return Err(SelectError::NoColumns);
    }
    validate(table, edit)?;
    let relation = dialect.qualified(&table.table.schema, &table.table.name)?;
    let mut binder = Binder {
        dialect,
        params: Vec::new(),
    };
    let sql = match edit.kind {
        EditKind::Insert => insert_sql(dialect, &relation, &binder.assignments(edit)?),
        EditKind::Update => format!(
            "UPDATE {relation} SET {} WHERE {}",
            binder
                .assignments(edit)?
                .iter()
                .map(|(name, value)| format!("{name} = {value}"))
                .collect::<Vec<_>>()
                .join(", "),
            binder.guard(edit)?
        ),
        EditKind::Delete => format!("DELETE FROM {relation} WHERE {}", binder.guard(edit)?),
    };
    Ok(BoundSql {
        sql,
        params: binder.params,
    })
}

/// Collects bind parameters in placeholder order.
struct Binder {
    dialect: Dialect,
    params: Vec<Option<String>>,
}

impl Binder {
    fn bind(&mut self, value: Option<&String>) -> Result<String, SelectError> {
        if value.is_some_and(|value| value.contains('\0')) {
            return Err(SelectError::NulCharacter);
        }
        self.params.push(value.cloned());
        Ok(self.dialect.placeholder(self.params.len()))
    }

    /// Quoted column and SQL value expression pairs; inserts leave DEFAULT columns out.
    fn assignments(&mut self, edit: &Edit) -> Result<Vec<(String, String)>, SelectError> {
        edit.values
            .iter()
            .filter(|(_, value)| edit.kind != EditKind::Insert || **value != CellValue::Default)
            .map(|(name, value)| {
                let expression = match value {
                    CellValue::Default if self.dialect == Dialect::Sqlite => {
                        return Err(SelectError::Unsupported(
                            "SQLite cannot set a column back to its default in an UPDATE",
                        ));
                    }
                    CellValue::Default => "DEFAULT".to_owned(),
                    CellValue::Null => self.bind(None)?,
                    CellValue::Text(text) => self.bind(Some(text))?,
                };
                Ok((self.dialect.quote_ident(name)?, expression))
            })
            .collect()
    }

    /// Every original column must still hold its snapshot text.
    fn guard(&mut self, edit: &Edit) -> Result<String, SelectError> {
        edit.original
            .iter()
            .map(|(name, value)| {
                let column = self.dialect.quote_ident(name)?;
                let placeholder = self.bind(value.as_ref())?;
                Ok(self.dialect.same_text(&column, &placeholder))
            })
            .collect::<Result<Vec<_>, SelectError>>()
            .map(|parts| parts.join(" AND "))
    }
}

fn insert_sql(dialect: Dialect, relation: &str, pairs: &[(String, String)]) -> String {
    if pairs.is_empty() {
        return match dialect {
            Dialect::MySql => format!("INSERT INTO {relation} () VALUES ()"),
            Dialect::Postgres | Dialect::Sqlite | Dialect::SqlServer => {
                format!("INSERT INTO {relation} DEFAULT VALUES")
            }
        };
    }
    let (names, values): (Vec<&str>, Vec<&str>) = pairs
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .unzip();
    format!(
        "INSERT INTO {relation} ({}) VALUES ({})",
        names.join(", "),
        values.join(", ")
    )
}
fn validate(table: &EditTable, edit: &Edit) -> Result<(), SelectError> {
    for name in edit.values.keys().chain(edit.original.keys()) {
        if !table.columns.contains(name) {
            return Err(SelectError::UnknownColumn(name.clone()));
        }
    }
    for name in edit.values.keys() {
        if table.protected.contains(name) {
            return Err(SelectError::UnknownColumn(name.clone()));
        }
    }
    if edit.kind != EditKind::Insert
        && (table
            .columns
            .iter()
            .any(|name| !edit.original.contains_key(name))
            || table
                .key
                .iter()
                .any(|name| edit.original.get(name).is_none_or(Option::is_none)))
    {
        return Err(SelectError::NoColumns);
    }
    if edit.kind == EditKind::Update && edit.values.is_empty() {
        return Err(SelectError::NoColumns);
    }
    if edit.kind == EditKind::Delete && !edit.values.is_empty() {
        return Err(SelectError::NoColumns);
    }
    if edit.kind == EditKind::Insert && !edit.original.is_empty() {
        return Err(SelectError::NoColumns);
    }
    Ok(())
}
#[cfg(test)]
mod tests;
