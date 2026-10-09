// SPDX-License-Identifier: Apache-2.0
//! The table rebuild for changes `ALTER TABLE` cannot make in `SQLite`.
use super::{DIALECT, Plan, data_type, default_sql, new_column, quote, relation};
use openrows_driver_api::schema::DefaultValue;
use openrows_driver_api::schema::{Generated, NewColumn, TableDetail};
use openrows_driver_api::structure::{AlterError, ColumnEdits, StagedChanges};
use openrows_sql::ddl::{ColumnDefinition, TableDefinition, TableElement, qualify_created_name};

/// Scope `SQLite`'s pre-3.26 rename behaviour to the staging table's rename.
pub(crate) const LEGACY_RENAME_ON: &str = "PRAGMA legacy_alter_table = ON";
pub(crate) const LEGACY_RENAME_OFF: &str = "PRAGMA legacy_alter_table = OFF";

/// Prefix of the new table while the rows are copied.
const STAGING: &str = "openrows_rebuild_";

/// True when some change needs the table rebuilt.
pub(super) fn needed(detail: &TableDetail, staged: &StagedChanges<'_>) -> bool {
    staged.columns.values().any(redefines)
        || staged.drops.iter().any(|name| constrained(detail, name))
        || staged.adds.iter().any(|column| !addable(column))
}

fn redefines(edits: &ColumnEdits<'_>) -> bool {
    edits.data_type.is_some() || edits.nullable.is_some() || edits.default.is_some()
}

/// `DROP COLUMN` refuses key, unique, indexed, foreign-key and generated columns.
fn constrained(detail: &TableDetail, name: &str) -> bool {
    detail
        .constraints
        .iter()
        .any(|constraint| constraint.columns.iter().any(|column| column == name))
        || detail
            .indexes
            .iter()
            .any(|index| index.columns.split(", ").any(|column| column == name))
        || detail
            .columns
            .iter()
            .any(|column| column.name == name && column.generated != Generated::No)
}

/// `ADD COLUMN` needs a constant default, and a non-NULL one for `NOT NULL`.
fn addable(column: &NewColumn) -> bool {
    match column.default {
        DefaultValue::Expression(_) => false,
        DefaultValue::None => column.nullable,
        DefaultValue::Literal(_) => true,
    }
}

/// Append the rebuild statements to `plan`.
pub(super) fn write(plan: &mut Plan<'_>) -> Result<(), AlterError> {
    let (target, staged) = (plan.target, plan.staged);
    let original = target
        .detail
        .info
        .as_ref()
        .and_then(|info| info.definition.as_deref())
        .and_then(|sql| TableDefinition::parse(sql, DIALECT))
        .ok_or_else(|| AlterError::UnreadableDefinition(target.table.to_owned()))?;
    let staging = relation(target.schema, &format!("{STAGING}{}", target.table))?;
    let table = TableDefinition {
        head: format!("CREATE TABLE {staging} ("),
        elements: elements(plan, &original)?,
        tail: original.tail.clone(),
    };
    let current = plan.relation()?;
    let copied = copied_columns(target.detail, staged)?;
    plan.push(table.render());
    plan.push(format!(
        "INSERT INTO {staging} ({copied}) SELECT {copied} FROM {current}"
    ));
    plan.push(format!("DROP TABLE {current}"));
    // Views and triggers naming the table would fail the rename while the
    // table is gone; the legacy rename leaves them alone (they name it again).
    plan.push(LEGACY_RENAME_ON.to_owned());
    plan.push(format!(
        "ALTER TABLE {staging} RENAME TO {}",
        quote(target.table)?
    ));
    plan.push(LEGACY_RENAME_OFF.to_owned());
    recreate(plan)?;
    plan.warn(
        "SQLite cannot make these changes in place: the table is rebuilt, its rows copied, \
         and its indexes and triggers created again in one transaction."
            .to_owned(),
    );
    Ok(())
}

/// The definitions of the new table: dropped columns left out, changed
/// ones rewritten in place, new ones after the last column.
fn elements(
    plan: &mut Plan<'_>,
    original: &TableDefinition,
) -> Result<Vec<TableElement>, AlterError> {
    let staged = plan.staged;
    let mut elements = Vec::new();
    let mut after_columns = 0;
    for element in &original.elements {
        let Some(name) = element.column.as_deref() else {
            elements.push(element.clone());
            continue;
        };
        if staged.drops.contains(&name) {
            plan.warn(format!(
                "Dropping column {name:?} permanently deletes its data."
            ));
            plan.out.destructive = true;
            continue;
        }
        let mut element = element.clone();
        if let Some(edits) = staged.columns.get(name).filter(|edits| redefines(edits)) {
            let (leading, body, trailing) = element.parts(DIALECT);
            let text = format!("{leading}{}{trailing}", redefine(plan, name, body, edits)?);
            element.text = text;
        }
        elements.push(element);
        after_columns = elements.len();
    }
    for column in &staged.adds {
        let mut added = TableElement {
            text: format!("\n    {}", new_column(column)?),
            column: Some(column.name.clone()),
        };
        // At the end of the list the closing line break moves to the new column.
        if after_columns == elements.len()
            && let Some(last) = elements.last_mut()
        {
            let (leading, body, trailing) = last.parts(DIALECT);
            added.text.push_str(trailing);
            last.text = format!("{leading}{body}");
        }
        elements.insert(after_columns, added);
        after_columns += 1;
    }
    Ok(elements)
}

fn redefine(
    plan: &mut Plan<'_>,
    name: &str,
    text: &str,
    edits: &ColumnEdits<'_>,
) -> Result<String, AlterError> {
    let mut definition = ColumnDefinition::parse(text, DIALECT)
        .ok_or_else(|| AlterError::UnreadableDefinition(plan.target.table.to_owned()))?;
    if let Some(text) = edits.data_type {
        definition.set_type(&data_type(text)?);
        plan.warn(format!(
            "Values of {name:?} are converted to the new type's affinity where SQLite can; others keep their value."
        ));
        plan.out.destructive = true;
    }
    if let Some(nullable) = edits.nullable {
        definition.set_nullable(nullable);
        if !nullable {
            plan.warn(format!(
                "NOT NULL on {name:?} fails if the column contains NULLs."
            ));
        }
    }
    if let Some(default) = edits.default {
        definition.set_default(default_sql(name, default)?.as_deref());
    }
    Ok(definition.render())
}

/// Stored columns that survive, quoted and comma-separated.
fn copied_columns(detail: &TableDetail, staged: &StagedChanges<'_>) -> Result<String, AlterError> {
    Ok(detail
        .columns
        .iter()
        .filter(|column| column.generated == Generated::No)
        .filter(|column| !staged.drops.contains(&column.name.as_str()))
        .map(|column| quote(&column.name))
        .collect::<Result<Vec<_>, _>>()?
        .join(", "))
}

/// The table's own indexes (unless dropped or on a dropped column) and
/// triggers, created again in its schema.
fn recreate(plan: &mut Plan<'_>) -> Result<(), AlterError> {
    let (target, staged) = (plan.target, plan.staged);
    let schema = quote(target.schema)?;
    let indexes = target.detail.indexes.iter().filter(|index| {
        !index.definition.starts_with("--")
            && !staged.index_drops.contains(&index.name.as_str())
            && !index
                .columns
                .split(", ")
                .any(|column| staged.drops.contains(&column))
    });
    let definitions: Vec<&str> = indexes
        .map(|index| index.definition.as_str())
        .chain(
            target
                .detail
                .triggers
                .iter()
                .map(|trigger| trigger.definition.as_str()),
        )
        .collect();
    for sql in definitions {
        let qualified = qualify_created_name(sql, DIALECT, &schema)
            .ok_or_else(|| AlterError::UnreadableDefinition(target.table.to_owned()))?;
        plan.push(qualified);
    }
    Ok(())
}
