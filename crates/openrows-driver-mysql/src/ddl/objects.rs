// SPDX-License-Identifier: Apache-2.0
//! Object lifecycle changes as `MySQL` DDL (FR-SCHEMA-10, 11, 12). `MySQL`
//! commits every DDL statement at once, so a multi-statement plan says so;
//! a database is its own only schema, and databases cannot be renamed.
use super::keys::{self, charset_name, column_definition, column_list};
use super::{DIALECT, literal, quote};
use openrows_driver_api::GenerateError;
use openrows_driver_api::objects::{
    NewTable, ObjectChange, ObjectName, ObjectPlan, PlanRun, definition_statements, view_query,
};
use openrows_driver_api::schema::{Generated, TableDetail};
use openrows_driver_api::structure::AlterError;
use std::fmt::Write as _;

/// Databases the server needs; never dropped from here.
const SYSTEM: [&str; 4] = ["mysql", "information_schema", "performance_schema", "sys"];

const NOT_ATOMIC: &str =
    "MySQL commits each statement at once: if a later one fails, the earlier ones stay.";

/// The plan for `change`; `source` is the duplicated table's detail.
///
/// # Errors
/// Invalid names and definitions, and what `MySQL` cannot do.
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
                "RENAME TABLE {} TO {}",
                name_of(table)?,
                name_of(&ObjectName {
                    schema: table.schema.clone(),
                    name: name.clone(),
                })?
            )],
            &["Code that uses the old table name stops working."],
        )),
        ObjectChange::DuplicateTable {
            table,
            name,
            with_data,
        } => duplicate(table, name, *with_data, source),
        ObjectChange::TruncateTable { table, cascade } => {
            no_cascade(*cascade)?;
            Ok(destructive(
                vec![format!("TRUNCATE TABLE {}", name_of(table)?)],
                &[
                    "Every row is deleted; TRUNCATE commits at once and cannot be rolled back.",
                    "It fails while another table's foreign key references this one.",
                ],
            ))
        }
        ObjectChange::DropTable { table, cascade } => {
            no_cascade(*cascade)?;
            Ok(destructive(
                vec![format!("DROP TABLE {}", name_of(table)?)],
                &["The table and every row in it are deleted permanently."],
            ))
        }
        other => views_and_routines(other),
    }
}

fn views_and_routines(change: &ObjectChange) -> Result<ObjectPlan, GenerateError> {
    match change {
        ObjectChange::SaveView {
            view,
            query,
            materialized,
            replace,
        } => {
            if *materialized {
                return Err(GenerateError("MySQL has no materialized views".to_owned()));
            }
            let query = view_query(query, DIALECT).map_err(|e| GenerateError(e.to_string()))?;
            let or_replace = if *replace { "OR REPLACE " } else { "" };
            Ok(plan(
                vec![format!(
                    "CREATE {or_replace}VIEW {} AS\n{query}",
                    name_of(view)?
                )],
                &[],
            ))
        }
        ObjectChange::DropView { view, cascade, .. } => {
            no_cascade(*cascade)?;
            Ok(destructive(
                vec![format!("DROP VIEW {}", name_of(view)?)],
                &["Queries and views that use it stop working."],
            ))
        }
        ObjectChange::SaveTrigger {
            table,
            replaces,
            definition,
        } => {
            let old = replaces
                .as_ref()
                .map(|name| drop_if_exists("TRIGGER", &table.schema, name))
                .transpose()?;
            Ok(replaced(old, checked(definition, "TRIGGER")?))
        }
        ObjectChange::DropTrigger { table, name } => Ok(destructive(
            vec![format!("DROP TRIGGER {}", in_schema(&table.schema, name)?)],
            &["The trigger no longer runs on changes to the table."],
        )),
        ObjectChange::SaveRoutine {
            routine,
            kind,
            replaces,
            definition,
        } => {
            let old = replaces
                .as_ref()
                .map(|_| drop_if_exists(kind.keyword(), &routine.schema, &routine.name))
                .transpose()?;
            Ok(replaced(old, checked(definition, kind.keyword())?))
        }
        ObjectChange::DropRoutine { routine, kind, .. } => Ok(destructive(
            vec![format!("DROP {} {}", kind.keyword(), name_of(routine)?)],
            &["Code that calls it stops working."],
        )),
        other => databases(other),
    }
}

fn databases(change: &ObjectChange) -> Result<ObjectPlan, GenerateError> {
    match change {
        ObjectChange::CreateDatabase {
            name,
            encoding,
            collation,
        } => {
            let mut sql = format!("CREATE DATABASE {}", ident(name)?);
            if let Some(charset) =
                charset_name(encoding.as_deref()).map_err(|error| fail(&error))?
            {
                let _ = write!(sql, " CHARACTER SET {charset}");
            }
            if let Some(collation) =
                charset_name(collation.as_deref()).map_err(|error| fail(&error))?
            {
                let _ = write!(sql, " COLLATE {collation}");
            }
            Ok(on_home(sql, &[], false))
        }
        ObjectChange::DropDatabase { name } => {
            if SYSTEM.contains(&name.to_ascii_lowercase().as_str()) {
                return Err(GenerateError(format!(
                    "{name} is a system database and cannot be dropped here"
                )));
            }
            Ok(on_home(
                format!("DROP DATABASE {}", ident(name)?),
                &["Every table, view, routine and row in it is deleted permanently."],
                true,
            ))
        }
        ObjectChange::RenameDatabase { .. } => Err(GenerateError(
            "MySQL cannot rename a database; create a new one and rename each table into it"
                .to_owned(),
        )),
        _ => Err(GenerateError(
            "In MySQL a schema is a database: create or drop the database instead".to_owned(),
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
    Ok(format!("{}.{}", ident(schema)?, ident(name)?))
}

fn name_of(object: &ObjectName) -> Result<String, GenerateError> {
    in_schema(&object.schema, &object.name)
}

fn no_cascade(cascade: bool) -> Result<(), GenerateError> {
    if cascade {
        return Err(GenerateError(
            "MySQL has no CASCADE here; drop the dependent objects first".to_owned(),
        ));
    }
    Ok(())
}

fn drop_if_exists(kind: &str, schema: &str, name: &str) -> Result<String, GenerateError> {
    Ok(format!(
        "DROP {kind} IF EXISTS {}",
        in_schema(schema, name)?
    ))
}

fn checked(definition: &str, kind: &str) -> Result<Vec<String>, GenerateError> {
    definition_statements(definition, DIALECT, (&[], kind))
        .map_err(|error| GenerateError(error.to_string()))
}

/// The new definition, after dropping the old object when editing.
fn replaced(old: Option<String>, created: Vec<String>) -> ObjectPlan {
    let warnings: &[&str] = if old.is_some() { &[NOT_ATOMIC] } else { &[] };
    plan(old.into_iter().chain(created).collect(), warnings)
}

fn plan(statements: Vec<String>, warnings: &[&str]) -> ObjectPlan {
    ObjectPlan {
        statements,
        warnings: warnings
            .iter()
            .map(|warning| (*warning).to_owned())
            .collect(),
        destructive: false,
        run: PlanRun::Transaction,
    }
}

fn destructive(statements: Vec<String>, warnings: &[&str]) -> ObjectPlan {
    ObjectPlan {
        destructive: true,
        ..plan(statements, warnings)
    }
}

fn on_home(sql: String, warnings: &[&str], destroys: bool) -> ObjectPlan {
    ObjectPlan {
        destructive: destroys,
        run: PlanRun::HomeDatabase,
        ..plan(vec![sql], warnings)
    }
}

fn create_table(table: &ObjectName, definition: &NewTable) -> Result<ObjectPlan, AlterError> {
    definition.validate()?;
    let mut lines = definition
        .columns
        .iter()
        .map(column_definition)
        .collect::<Result<Vec<_>, _>>()?;
    if !definition.primary_key.is_empty() {
        lines.push(format!(
            "PRIMARY KEY ({})",
            column_list(&definition.primary_key)?
        ));
    }
    for index in &definition.indexes {
        lines.push(keys::index_spec(index)?);
    }
    for key in &definition.foreign_keys {
        lines.push(keys::foreign_key(key, &table.schema)?);
    }
    for check in &definition.checks {
        lines.push(keys::check(check)?);
    }
    let name = format!("{}.{}", quote(&table.schema)?, quote(&table.name)?);
    let mut sql = format!("CREATE TABLE {name} (\n  {}\n)", lines.join(",\n  "));
    if let Some(comment) = &definition.comment {
        let _ = write!(sql, " COMMENT = {}", literal(comment)?);
    }
    Ok(plan(vec![sql], &[]))
}

fn duplicate(
    table: &ObjectName,
    name: &str,
    with_data: bool,
    source: Option<&TableDetail>,
) -> Result<ObjectPlan, GenerateError> {
    let from = name_of(table)?;
    let copy = in_schema(&table.schema, name)?;
    let mut statements = vec![format!("CREATE TABLE {copy} LIKE {from}")];
    let mut warnings = vec!["Foreign keys and triggers are not copied.".to_owned()];
    if with_data {
        let detail = source.ok_or_else(|| {
            GenerateError(format!("the structure of {} could not be read", table.name))
        })?;
        let columns: Vec<String> = detail
            .columns
            .iter()
            .filter(|column| column.generated != Generated::Stored)
            .map(|column| column.name.clone())
            .collect();
        let list = column_list(&columns).map_err(|error| fail(&error))?;
        statements.push(format!(
            "INSERT INTO {copy} ({list}) SELECT {list} FROM {from}"
        ));
        warnings.push(NOT_ATOMIC.to_owned());
    }
    Ok(ObjectPlan {
        warnings,
        ..plan(statements, &[])
    })
}

#[cfg(test)]
#[path = "objects_tests.rs"]
mod tests;
