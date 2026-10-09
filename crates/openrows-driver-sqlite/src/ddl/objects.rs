// SPDX-License-Identifier: Apache-2.0
//! Object lifecycle changes as `SQLite` DDL (FR-SCHEMA-10, 11). Everything
//! runs in one transaction. `SQLite` has no `TRUNCATE`, no `OR REPLACE`
//! for views and triggers (they are dropped and created again), no stored
//! routines, and databases are files (attached ones are the schemas).
use super::constraints;
use super::{DIALECT, collation_name, data_type, default_sql, quote, relation};
use openrows_driver_api::GenerateError;
use openrows_driver_api::objects::{
    NewTable, ObjectChange, ObjectName, ObjectPlan, PlanRun, definition_statements, view_query,
};
use openrows_driver_api::schema::{Generated, Identity, NewColumn, TableDetail};
use openrows_driver_api::structure::AlterError;
use openrows_sql::ddl::{ColumnDefinition, TableDefinition};
use std::fmt::Write as _;

/// The plan for `change`; `source` is the duplicated table's detail.
///
/// # Errors
/// Invalid names and definitions, and what `SQLite` cannot do.
pub(crate) fn object_sql(
    change: &ObjectChange,
    source: Option<&TableDetail>,
) -> Result<ObjectPlan, GenerateError> {
    match change {
        ObjectChange::CreateTable { table, definition } => {
            create_table(table, definition).map_err(|error| fail(&error))
        }
        ObjectChange::RenameTable { table, name } => Ok(plan(
            vec![format!(
                "ALTER TABLE {} RENAME TO {}",
                name_of(table)?,
                ident(name)?
            )],
            &["Code that uses the old table name stops working."],
            false,
        )),
        ObjectChange::DuplicateTable {
            table,
            name,
            with_data,
        } => duplicate(table, name, *with_data, source).map_err(|error| fail(&error)),
        ObjectChange::TruncateTable { table, cascade } => {
            no_cascade(*cascade)?;
            Ok(plan(
                vec![format!("DELETE FROM {}", name_of(table)?)],
                &["SQLite has no TRUNCATE: every row is deleted with DELETE."],
                true,
            ))
        }
        ObjectChange::DropTable { table, cascade } => {
            no_cascade(*cascade)?;
            Ok(plan(
                vec![format!("DROP TABLE {}", name_of(table)?)],
                &["The table and every row in it are deleted permanently."],
                true,
            ))
        }
        other => views_and_triggers(other),
    }
}

fn views_and_triggers(change: &ObjectChange) -> Result<ObjectPlan, GenerateError> {
    match change {
        ObjectChange::SaveView {
            view,
            query,
            materialized,
            replace,
        } => {
            if *materialized {
                return Err(GenerateError(
                    "SQLite has no materialized views".to_owned(),
                ));
            }
            let query = view_query(query, DIALECT).map_err(|e| GenerateError(e.to_string()))?;
            let name = name_of(view)?;
            let old = replace.then(|| format!("DROP VIEW IF EXISTS {name}"));
            let create = format!("CREATE VIEW {name} AS\n{query}");
            Ok(plan(old.into_iter().chain([create]).collect(), &[], false))
        }
        ObjectChange::DropView { view, cascade, .. } => {
            no_cascade(*cascade)?;
            Ok(plan(
                vec![format!("DROP VIEW {}", name_of(view)?)],
                &["Queries and triggers that use it stop working."],
                true,
            ))
        }
        ObjectChange::SaveTrigger {
            table,
            replaces,
            definition,
        } => {
            let old = replaces
                .as_ref()
                .map(|name| in_schema(&table.schema, name))
                .transpose()?
                .map(|name| format!("DROP TRIGGER IF EXISTS {name}"));
            let created = definition_statements(definition, DIALECT, (&[], "TRIGGER"))
                .map_err(|error| GenerateError(error.to_string()))?;
            Ok(plan(old.into_iter().chain(created).collect(), &[], false))
        }
        ObjectChange::DropTrigger { table, name } => Ok(plan(
            vec![format!("DROP TRIGGER {}", in_schema(&table.schema, name)?)],
            &["The trigger no longer runs on changes to the table."],
            true,
        )),
        ObjectChange::SaveRoutine { .. } | ObjectChange::DropRoutine { .. } => Err(
            GenerateError("SQLite has no stored functions or procedures".to_owned()),
        ),
        _ => Err(GenerateError(
            "SQLite databases are files: create one with New connection, and attach files to add schemas"
                .to_owned(),
        )),
    }
}

fn fail(error: &AlterError) -> GenerateError {
    GenerateError(error.to_string())
}

fn ident(name: &str) -> Result<String, GenerateError> {
    quote(name).map_err(|error| fail(&error))
}

fn in_schema(schema: &str, name: &str) -> Result<String, GenerateError> {
    relation(schema, name).map_err(|error| fail(&error))
}

fn name_of(object: &ObjectName) -> Result<String, GenerateError> {
    in_schema(&object.schema, &object.name)
}

fn no_cascade(cascade: bool) -> Result<(), GenerateError> {
    if cascade {
        return Err(GenerateError("SQLite has no CASCADE here".to_owned()));
    }
    Ok(())
}

fn plan(statements: Vec<String>, warnings: &[&str], destructive: bool) -> ObjectPlan {
    ObjectPlan {
        statements,
        warnings: warnings
            .iter()
            .map(|warning| (*warning).to_owned())
            .collect(),
        destructive,
        run: PlanRun::Transaction,
    }
}

/// `name type [COLLATE …] [NOT NULL] [DEFAULT …]`.
fn column_definition(column: &NewColumn) -> Result<String, AlterError> {
    if column.identity != Identity::None {
        return Err(AlterError::Unsupported(super::NUMBERING.to_owned()));
    }
    if column.comment.is_some() || column.charset.is_some() {
        return Err(AlterError::Unsupported(
            "SQLite columns have no comments or character sets".to_owned(),
        ));
    }
    let mut definition =
        ColumnDefinition::new(&quote(&column.name)?, &data_type(&column.data_type)?);
    definition.set_collation(None, collation_name(column.collation.as_deref())?);
    definition.set_nullable(column.nullable);
    definition.set_default(default_sql(&column.name, &column.default)?.as_deref());
    Ok(definition.render())
}

fn create_table(table: &ObjectName, definition: &NewTable) -> Result<ObjectPlan, AlterError> {
    definition.validate()?;
    if definition.comment.is_some() {
        return Err(AlterError::Unsupported(
            "SQLite tables have no comments".to_owned(),
        ));
    }
    let columns = |names: &[String]| -> Result<String, AlterError> {
        Ok(names
            .iter()
            .map(|name| quote(name))
            .collect::<Result<Vec<_>, _>>()?
            .join(", "))
    };
    let mut lines = definition
        .columns
        .iter()
        .map(column_definition)
        .collect::<Result<Vec<_>, _>>()?;
    if !definition.primary_key.is_empty() {
        lines.push(format!(
            "PRIMARY KEY ({})",
            columns(&definition.primary_key)?
        ));
    }
    for key in &definition.foreign_keys {
        lines.push(constraints::foreign_key(key)?);
    }
    for check in &definition.checks {
        lines.push(constraints::check(check)?);
    }
    let name = relation(&table.schema, &table.name)?;
    let mut statements = vec![format!(
        "CREATE TABLE {name} (\n    {}\n)",
        lines.join(",\n    ")
    )];
    for index in &definition.indexes {
        let index_name = index
            .name
            .clone()
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| format!("{}_{}_idx", table.name, index.columns.join("_")));
        let unique = if index.unique { "UNIQUE " } else { "" };
        let keys = index
            .columns
            .iter()
            .map(|column| {
                let order = if index.descending.contains(column) {
                    " DESC"
                } else {
                    ""
                };
                quote(column).map(|quoted| format!("{quoted}{order}"))
            })
            .collect::<Result<Vec<_>, _>>()?
            .join(", ");
        let mut sql = format!(
            "CREATE {unique}INDEX {} ON {} ({keys})",
            relation(&table.schema, &index_name)?,
            quote(&table.name)?
        );
        if let Some(predicate) = index.predicate.as_deref().map(str::trim) {
            let _ = write!(sql, " WHERE ({predicate})");
        }
        statements.push(sql);
    }
    Ok(plan(statements, &[], false))
}

/// The stored definition under the new name, then the rows when asked.
fn duplicate(
    table: &ObjectName,
    name: &str,
    with_data: bool,
    source: Option<&TableDetail>,
) -> Result<ObjectPlan, AlterError> {
    let unreadable = || AlterError::UnreadableDefinition(table.name.clone());
    let detail = source.ok_or_else(unreadable)?;
    let original = detail
        .info
        .as_ref()
        .and_then(|info| info.definition.as_deref())
        .and_then(|sql| TableDefinition::parse(sql, DIALECT))
        .ok_or_else(unreadable)?;
    let copy = relation(&table.schema, name)?;
    let created = TableDefinition {
        head: format!("CREATE TABLE {copy} ("),
        ..original
    };
    let mut statements = vec![created.render()];
    if with_data {
        let columns = detail
            .columns
            .iter()
            .filter(|column| column.generated == Generated::No)
            .map(|column| quote(&column.name))
            .collect::<Result<Vec<_>, _>>()?
            .join(", ");
        statements.push(format!(
            "INSERT INTO {copy} ({columns}) SELECT {columns} FROM {}",
            relation(&table.schema, &table.name)?
        ));
    }
    Ok(plan(
        statements,
        &["Indexes and triggers are not copied."],
        false,
    ))
}

#[cfg(test)]
#[path = "objects_tests.rs"]
mod tests;
