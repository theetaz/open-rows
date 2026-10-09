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
