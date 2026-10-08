// SPDX-License-Identifier: Apache-2.0
//! Parameterised, optimistic table edits; identifiers come from catalog metadata.
use crate::select::{BoundSql, SelectError, TableRef, quote_ident};
use std::collections::BTreeMap;

/// Exact `PostgreSQL` text values, never display-formatted or rounded values.
pub type Values = BTreeMap<String, Option<String>>;
/// Staged field values by column name.
pub type Changes = BTreeMap<String, CellValue>;
/// One staged field value: NULL, the column default, or exact `PostgreSQL` text input.
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
/// `INSERT` column list (all omitted: `DEFAULT VALUES`); they never become parameters.
///
/// # Errors
/// Invalid identifiers, NUL data, unknown columns or a missing stable row identity.
pub fn edit_sql(table: &EditTable, edit: &Edit) -> Result<BoundSql, SelectError> {
    if table.key.is_empty() || table.columns.is_empty() {
        return Err(SelectError::NoColumns);
    }
    validate(table, edit)?;
    let relation = format!(
        "{}.{}",
        quote_ident(&table.table.schema)?,
        quote_ident(&table.table.name)?
    );
    let mut params = Vec::new();
    let sql = match edit.kind {
        EditKind::Insert => insert_sql(&relation, &assignments(edit, &mut params)?),
        EditKind::Update => format!(
            "UPDATE {relation} SET {} WHERE {}",
            assignments(edit, &mut params)?
                .iter()
                .map(|(name, value)| format!("{name} = {value}"))
                .collect::<Vec<_>>()
                .join(", "),
            guard(edit, &mut params)?
        ),
        EditKind::Delete => format!("DELETE FROM {relation} WHERE {}", guard(edit, &mut params)?),
    };
    Ok(BoundSql { sql, params })
}
/// Quoted column and SQL value expression pairs; inserts leave DEFAULT columns out.
fn assignments(
    edit: &Edit,
    params: &mut Vec<Option<String>>,
) -> Result<Vec<(String, String)>, SelectError> {
    edit.values
        .iter()
        .filter(|(_, value)| edit.kind != EditKind::Insert || **value != CellValue::Default)
        .map(|(name, value)| {
            let expression = match value {
                CellValue::Default => "DEFAULT".to_owned(),
                CellValue::Null => bind(params, None)?,
                CellValue::Text(text) => bind(params, Some(text))?,
            };
            Ok((quote_ident(name)?, expression))
        })
        .collect()
}
fn insert_sql(relation: &str, pairs: &[(String, String)]) -> String {
    if pairs.is_empty() {
        return format!("INSERT INTO {relation} DEFAULT VALUES");
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
fn bind(params: &mut Vec<Option<String>>, value: Option<&String>) -> Result<String, SelectError> {
    if value.is_some_and(|value| value.contains('\0')) {
        return Err(SelectError::NulCharacter);
    }
    params.push(value.cloned());
    Ok(format!("${}", params.len()))
}
fn guard(edit: &Edit, params: &mut Vec<Option<String>>) -> Result<String, SelectError> {
    edit.original
        .iter()
        .map(|(name, value)| {
            Ok(format!(
                "{}::text IS NOT DISTINCT FROM {}::text",
                quote_ident(name)?,
                bind(params, value.as_ref())?
            ))
        })
        .collect::<Result<Vec<_>, SelectError>>()
        .map(|parts| parts.join(" AND "))
}
#[cfg(test)]
mod tests;
