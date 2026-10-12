// SPDX-License-Identifier: Apache-2.0
//! Checks of staged indexes, foreign keys, checks and constraint drops.
use crate::schema::{NewForeignKey, NewIndex, TableDetail};
use crate::structure::AlterError;
use std::collections::BTreeSet;

/// Key columns exist after the change set and descending ones are keys.
pub(crate) fn index(index: &NewIndex, finals: &BTreeSet<&str>) -> Result<(), AlterError> {
    if index.columns.is_empty() {
        return Err(AlterError::NoIndexColumns);
    }
    if let Some(name) = index
        .columns
        .iter()
        .chain(&index.descending)
        .find(|name| !finals.contains(name.as_str()) || !index.columns.contains(name))
    {
        return Err(AlterError::UnknownColumn(name.clone()));
    }
    if let Some(predicate) = &index.predicate {
        expression(predicate)?;
    }
    Ok(())
}

/// Columns exist, and as many referenced columns as columns are named.
pub(crate) fn foreign_key(key: &NewForeignKey, finals: &BTreeSet<&str>) -> Result<(), AlterError> {
    let label = key.name.clone().unwrap_or_else(|| key.columns.join(", "));
    if key.columns.is_empty()
        || key.columns.len() != key.target.columns.len()
        || key.target.table.is_empty()
    {
        return Err(AlterError::InvalidForeignKey(label));
    }
    if let Some(name) = key
        .columns
        .iter()
        .find(|name| !finals.contains(name.as_str()))
    {
        return Err(AlterError::UnknownColumn(name.clone()));
    }
    Ok(())
}

/// A non-empty expression without NUL characters.
pub(crate) fn expression(sql: &str) -> Result<(), AlterError> {
    if sql.trim().is_empty() || sql.contains('\0') {
        return Err(AlterError::InvalidExpression(sql.to_owned()));
    }
    Ok(())
}

/// Every dropped constraint is in the catalog.
pub(crate) fn constraint_drops(drops: &[&str], detail: &TableDetail) -> Result<(), AlterError> {
    match drops
        .iter()
        .find(|name| !detail.constraints.iter().any(|c| c.name == **name))
    {
        Some(name) => Err(AlterError::UnknownConstraint((*name).to_owned())),
        None => Ok(()),
    }
}

/// Keys must refer to distinct final columns and have distinct names.
pub(crate) fn new_keys(
    staged: &super::StagedChanges<'_>,
    detail: &TableDetail,
    finals: &BTreeSet<&str>,
) -> Result<(), AlterError> {
    let kept = detail
        .constraints
        .iter()
        .filter(|c| !staged.constraint_drops.contains(&c.name.as_str()));
    let primary_count = kept
        .clone()
        .filter(|c| c.kind == crate::schema::ConstraintKind::Primary)
        .count();
    if primary_count + staged.primary_keys.len() > 1 {
        return Err(AlterError::Conflict("primary key".into()));
    }
    for (&name, edits) in &staged.columns {
        if edits.nullable == Some(true) && staged.primary_column(name) {
            return Err(AlterError::Conflict(format!(
                "primary-key column {name} cannot allow NULL"
            )));
        }
    }
    let mut names: BTreeSet<&str> = kept.map(|c| c.name.as_str()).collect();
    for name in staged
        .foreign_keys
        .iter()
        .filter_map(|key| key.name.as_deref())
        .chain(
            staged
                .checks
                .iter()
                .filter_map(|check| check.name.as_deref()),
        )
    {
        if !names.insert(name) {
            return Err(AlterError::Conflict(name.into()));
        }
    }
    for key in staged.primary_keys.iter().chain(&staged.unique_keys) {
        key_columns(&key.columns, finals)?;
        if let Some(name) = &key.name {
            key_name(name, &mut names)?;
        }
    }
    Ok(())
}

fn key_columns(columns: &[String], finals: &BTreeSet<&str>) -> Result<(), AlterError> {
    if columns.is_empty() {
        return Err(AlterError::NoKeyColumns);
    }
    let mut distinct = BTreeSet::new();
    for column in columns {
        if !finals.contains(column.as_str()) {
            return Err(AlterError::UnknownColumn(column.clone()));
        }
        if !distinct.insert(column) {
            return Err(AlterError::Conflict(column.clone()));
        }
    }
    Ok(())
}

fn key_name<'a>(name: &'a str, names: &mut BTreeSet<&'a str>) -> Result<(), AlterError> {
    if name.is_empty() || name.contains('\0') {
        return Err(AlterError::InvalidName);
    }
    if !names.insert(name) {
        return Err(AlterError::Conflict(name.into()));
    }
    Ok(())
}

/// Position anchors must survive the change set; self-placement is refused.
pub(crate) fn reorders(
    staged: &super::StagedChanges<'_>,
    current: &BTreeSet<&str>,
) -> Result<(), AlterError> {
    for &(column, after) in &staged.reorders {
        for name in std::iter::once(column).chain(after) {
            if !current.contains(name) || staged.drops.contains(&name) {
                return Err(AlterError::UnknownColumn(name.to_owned()));
            }
        }
        if after == Some(column) {
            return Err(AlterError::Conflict(column.to_owned()));
        }
    }
    Ok(())
}
