// SPDX-License-Identifier: Apache-2.0
//! Table constraints of a stored `CREATE TABLE` for a rebuild: the checks
//! the catalog lists (`SQLite` has no pragma for them), new foreign keys
//! and checks, and the elements a dropped constraint removes.
use super::{DIALECT, quote};
use openrows_driver_api::schema::{
    ConstraintDetail, ConstraintKind, NewCheck, NewForeignKey, ReferentialAction,
};
use openrows_driver_api::structure::AlterError;
use openrows_sql::ddl::{ColumnDefinition, TableDefinition, TableElement};
use std::fmt::Write as _;

/// A table-level check: its name (declared, else `ck_<table>_<n>`), the
/// element index and the `CHECK (…)` text.
pub(crate) struct TableCheck {
    pub(crate) name: String,
    pub(crate) element: usize,
    pub(crate) text: String,
}

/// The first two significant words of an element, upper-case.
fn words(element: &TableElement) -> (String, String, String) {
    let (_, body, _) = element.parts(DIALECT);
    let mut parts = body.split_whitespace();
    let first = parts.next().unwrap_or_default().to_ascii_uppercase();
    let second = parts.next().unwrap_or_default().to_owned();
    let third = parts.next().unwrap_or_default().to_ascii_uppercase();
    (first, second, third)
}

/// The declared name of a `CONSTRAINT name …` table constraint.
fn declared_name(element: &TableElement) -> Option<String> {
    let (first, name, _) = words(element);
    (element.column.is_none() && first == "CONSTRAINT").then(|| unquote(&name))
}

fn unquote(name: &str) -> String {
    let trimmed = name.trim_matches(|c| c == '"' || c == '`' || c == '[' || c == ']');
    trimmed.replace("\"\"", "\"")
}

/// The table-level checks of `definition`, in order.
pub(crate) fn table_checks(table: &str, definition: &TableDefinition) -> Vec<TableCheck> {
    let mut checks = Vec::new();
    for (index, element) in definition.elements.iter().enumerate() {
        if element.column.is_some() {
            continue;
        }
        let (first, _, third) = words(element);
        let check =
            first.starts_with("CHECK") || (first == "CONSTRAINT" && third.starts_with("CHECK"));
        if check {
            let name = declared_name(element)
                .unwrap_or_else(|| format!("ck_{table}_{}", checks.len() + 1));
            checks.push(TableCheck {
                name,
                element: index,
                text: element.parts(DIALECT).1.to_owned(),
            });
        }
    }
    checks
}

/// Remove the element (or column clause) behind `constraint` from `elements`.
///
/// # Errors
/// [`AlterError::Unsupported`] when no element can be matched.
pub(crate) fn drop_constraint(
    table: &str,
    elements: &mut Vec<TableElement>,
    constraint: &ConstraintDetail,
) -> Result<(), AlterError> {
    let original = TableDefinition {
        head: String::new(),
        elements: elements.clone(),
        tail: String::new(),
    };
    let named = elements
        .iter()
        .position(|element| declared_name(element).as_deref() == Some(constraint.name.as_str()));
    let checked = table_checks(table, &original)
        .into_iter()
        .find(|check| check.name == constraint.name)
        .map(|check| check.element);
    if let Some(index) = named
        .or(checked)
        .or_else(|| foreign_element(elements, constraint))
    {
        elements.remove(index);
        return Ok(());
    }
    if constraint.kind == ConstraintKind::Foreign
        && let [column] = constraint.columns.as_slice()
        && let Some(element) = elements
            .iter_mut()
            .find(|e| e.column.as_deref() == Some(column))
    {
        let (leading, body, trailing) = element.parts(DIALECT);
        let mut definition = ColumnDefinition::parse(body, DIALECT)
            .ok_or_else(|| AlterError::UnreadableDefinition(table.to_owned()))?;
        if definition.remove_references() {
            element.text = format!("{leading}{}{trailing}", definition.render());
            return Ok(());
        }
    }
    Err(AlterError::Unsupported(format!(
        "SQLite cannot drop {:?} here; only foreign keys, checks and named constraints can be dropped",
        constraint.name
    )))
}

/// A table-level `FOREIGN KEY (columns)` element of `constraint`.
fn foreign_element(elements: &[TableElement], constraint: &ConstraintDetail) -> Option<usize> {
    if constraint.kind != ConstraintKind::Foreign {
        return None;
    }
    let wanted: Vec<String> = constraint
        .columns
        .iter()
        .map(|c| c.to_ascii_lowercase())
        .collect();
    elements.iter().position(|element| {
        let upper = element.text.to_ascii_uppercase();
        let Some(at) = upper.find("FOREIGN KEY") else {
            return false;
        };
        let rest = &element.text[at + "FOREIGN KEY".len()..];
        let list = rest
            .split_once('(')
            .and_then(|(_, inside)| inside.split_once(')'))
            .map(|(inside, _)| inside)
            .unwrap_or_default();
        let columns: Vec<String> = list
            .split(',')
            .map(|column| unquote(column.trim()).to_ascii_lowercase())
            .collect();
        element.column.is_none() && columns == wanted
    })
}

/// `[CONSTRAINT n] FOREIGN KEY (…) REFERENCES t (…) [ON DELETE …] [ON UPDATE …]`
/// (the target must be in the same database).
pub(crate) fn foreign_key(key: &NewForeignKey) -> Result<String, AlterError> {
    let list = |columns: &[String]| -> Result<String, AlterError> {
        Ok(columns
            .iter()
            .map(|c| quote(c))
            .collect::<Result<Vec<_>, _>>()?
            .join(", "))
    };
    let mut sql = format!(
        "{}FOREIGN KEY ({}) REFERENCES {} ({})",
        named(key.name.as_deref())?,
        list(&key.columns)?,
        quote(&key.target.table)?,
        list(&key.target.columns)?
    );
    for (event, action) in [("DELETE", key.on_delete), ("UPDATE", key.on_update)] {
        if action != ReferentialAction::NoAction {
            let _ = write!(sql, " ON {event} {}", action.sql());
        }
    }
    Ok(sql)
}

/// `[CONSTRAINT n] CHECK (expression)`.
pub(crate) fn check(check: &NewCheck) -> Result<String, AlterError> {
    Ok(format!(
        "{}CHECK ({})",
        named(check.name.as_deref())?,
        check.expression.trim()
    ))
}

fn named(name: Option<&str>) -> Result<String, AlterError> {
    match name.filter(|name| !name.is_empty()) {
        Some(name) => Ok(format!("CONSTRAINT {} ", quote(name)?)),
        None => Ok(String::new()),
    }
}
