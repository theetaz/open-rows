// SPDX-License-Identifier: Apache-2.0
//! Reject source NULLs before an INTEGER PRIMARY KEY could assign new row ids.
use super::super::{Plan, quote};
use openrows_driver_api::schema::NewKey;
use openrows_driver_api::structure::{AlterError, StagedChanges};

pub(super) fn validate_primary_rows(plan: &mut Plan<'_>) -> Result<(), AlterError> {
    let staged = plan.staged;
    let mut columns = Vec::new();
    for key in &staged.primary_keys {
        for name in &key.columns {
            if let Some(check) = null_check(plan, name)? {
                columns.push(check);
            }
        }
    }
    if columns.is_empty() {
        return Ok(());
    }
    let guard = "\"temp\".\"openrows_primary_null_guard\"";
    plan.push(format!(
        "CREATE TEMP TABLE {guard} (value INTEGER NOT NULL)"
    ));
    plan.push(format!(
        "INSERT INTO {guard} SELECT NULL FROM {} WHERE {} LIMIT 1",
        plan.relation()?,
        columns.join(" OR ")
    ));
    plan.push(format!("DROP TABLE {guard}"));
    Ok(())
}

fn null_check(plan: &Plan<'_>, name: &str) -> Result<Option<String>, AlterError> {
    let staged = plan.staged;
    if staged.adds.iter().any(|column| column.name == name) {
        return Err(AlterError::Unsupported(
            "Add and populate new SQLite primary-key columns in a separate review before adding the key; otherwise INTEGER PRIMARY KEY can silently assign row IDs during the rebuild.".into(),
        ));
    }
    let current = staged
        .columns
        .iter()
        .find(|(_, edits)| edits.rename == Some(name))
        .map_or(name, |(&name, _)| name);
    if plan
        .target
        .detail
        .columns
        .iter()
        .any(|c| c.name == current && c.nullable)
    {
        Ok(Some(format!("{} IS NULL", quote(current)?)))
    } else {
        Ok(None)
    }
}

pub(super) fn append_keys(plan: &mut Plan<'_>, added: &mut Vec<String>) -> Result<(), AlterError> {
    let staged = plan.staged;
    for (keys, primary) in [(&staged.primary_keys, true), (&staged.unique_keys, false)] {
        for key in keys {
            let key = original_key(staged, key);
            added.push(openrows_driver_api::structure::key_sql(
                &key,
                primary,
                openrows_sql::Dialect::Sqlite,
            )?);
            plan.warn("Adding a key rebuilds the table; duplicate values or NULL primary-key values cause failure.".into());
        }
    }
    Ok(())
}

fn original_key(staged: &StagedChanges<'_>, key: &NewKey) -> NewKey {
    let mut key = key.clone();
    for name in &mut key.columns {
        if let Some((&current, _)) = staged
            .columns
            .iter()
            .find(|(_, edits)| edits.rename == Some(name.as_str()))
        {
            *name = current.to_owned();
        }
    }
    key
}
