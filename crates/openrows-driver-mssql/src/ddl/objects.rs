// SPDX-License-Identifier: Apache-2.0
//! Object lifecycle changes as T-SQL (FR-SCHEMA-10, 11, 12): tables, views,
//! triggers and routines in one transaction (SQL Server DDL is
//! transactional), databases one by one on `master`. Not available: schema
//! renames (SQL Server has none), materialized views and
//! `CASCADE` drops.
use openrows_driver_api::GenerateError;
use openrows_driver_api::objects::{
    NewTable, ObjectChange, ObjectName, ObjectPlan, PlanRun, definition_statements, view_query,
};
use openrows_driver_api::schema::{ColumnDetail, ConstraintKind, Generated, TableDetail};
use openrows_driver_api::structure::AlterError;
use openrows_sql::Dialect;

use super::definition::Relation;
use super::sql::{
    Level, check_spec, checked, collation, column_list, column_spec, comment_statement,
    foreign_key_spec, index_statement, literal,
};

/// Databases the server needs; never dropped from here.
const SYSTEM: [&str; 4] = ["master", "model", "msdb", "tempdb"];

/// The plan for `change`; `source` is the duplicated table's detail.
///
/// # Errors
/// Invalid names and definitions, and what SQL Server cannot do.
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
                "EXEC sys.sp_rename {}, {}",
                text(&name_of(table)?)?,
                text(name)?
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
                &["Every row is deleted; it fails while a foreign key references the table."],
            ))
        }
        ObjectChange::DropTable { table, cascade } => {
            no_cascade(*cascade)?;
            Ok(destructive(
                vec![format!("DROP TABLE {}", name_of(table)?)],
                &["The table and every row in it are deleted permanently."],
            ))
        }
        other => views_and_modules(other),
    }
}

fn views_and_modules(change: &ObjectChange) -> Result<ObjectPlan, GenerateError> {
    match change {
        ObjectChange::SaveView {
            view,
            query,
            materialized,
            replace,
        } => {
            if *materialized {
                return Err(GenerateError(
                    "SQL Server has no materialized views (indexed views are not created here)"
                        .to_owned(),
                ));
            }
            let query = view_query(query, Dialect::SqlServer)
                .map_err(|error| GenerateError(error.to_string()))?;
            let verb = if *replace {
                "CREATE OR ALTER"
            } else {
                "CREATE"
            };
            Ok(plan(
                vec![format!("{verb} VIEW {} AS\n{query}", name_of(view)?)],
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
            Ok(replaced(old, checked_definition(definition, "TRIGGER")?))
        }
        ObjectChange::DropTrigger { table, name } => Ok(destructive(
            vec![format!("DROP TRIGGER {}", in_schema(&table.schema, name)?)],
            &["The trigger no longer runs on changes to the table."],
        )),
        other => routines_and_databases(other),
    }
}

fn routines_and_databases(change: &ObjectChange) -> Result<ObjectPlan, GenerateError> {
    match change {
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
            Ok(replaced(
                old,
                checked_definition(definition, kind.keyword())?,
            ))
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
            collation: chosen,
        } => {
            if encoding.is_some() {
                return Err(GenerateError(
                    "SQL Server databases have no encoding; choose a collation (a _UTF8 one for UTF-8)"
                        .to_owned(),
                ));
            }
            let mut sql = format!("CREATE DATABASE {}", ident(name)?);
            if let Some(name) = chosen {
                sql.push_str(" COLLATE ");
                sql.push_str(&collation(name).map_err(|error| fail(&error))?);
            }
            Ok(on_home(sql, &[], false))
        }
        ObjectChange::DropDatabase { name } => {
            if SYSTEM.contains(&name.trim().to_ascii_lowercase().as_str()) {
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
        ObjectChange::RenameDatabase { name, to } => rename_database(name, to),
        ObjectChange::CreateSchema { name } => {
            Ok(plan(vec![format!("CREATE SCHEMA {}", ident(name)?)], &[]))
        }
        ObjectChange::DropSchema { name, cascade } => {
            no_cascade(*cascade)?;
            Ok(destructive(
                vec![format!("DROP SCHEMA {}", ident(name)?)],
                &["It fails while the schema still holds objects."],
            ))
        }
        ObjectChange::RenameSchema { .. } => Err(GenerateError(
            "SQL Server cannot rename a schema; create a new one and move the objects with \
             ALTER SCHEMA … TRANSFER"
                .to_owned(),
        )),
        _ => Err(GenerateError(
            "This SQL Server object operation is not supported here".to_owned(),
        )),
    }
}

fn rename_database(name: &str, to: &str) -> Result<ObjectPlan, GenerateError> {
    if [name, to]
        .iter()
        .any(|name| SYSTEM.contains(&name.trim().to_ascii_lowercase().as_str()))
    {
        return Err(GenerateError(
            "System databases cannot be renamed here.".into(),
        ));
    }
    Ok(on_home(
        format!(
            "ALTER DATABASE {} MODIFY NAME = {}",
            ident(name)?,
            ident(to)?
        ),
        &[
            "Database rename needs exclusive access and fails while other connections use it. Close them first; connections using the old name must be updated.",
        ],
        true,
    ))
}

fn fail(error: &AlterError) -> GenerateError {
    GenerateError(error.to_string())
}

fn ident(name: &str) -> Result<String, GenerateError> {
    checked(name).map_err(|error| fail(&error))
}

fn text(value: &str) -> Result<String, GenerateError> {
    literal(value).map_err(|error| fail(&error))
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
            "SQL Server has no CASCADE here; drop the dependent objects first".to_owned(),
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

/// The user's definition: exactly one `CREATE [OR ALTER] <kind>` statement.
fn checked_definition(definition: &str, kind: &str) -> Result<Vec<String>, GenerateError> {
    definition_statements(definition, Dialect::SqlServer, (&[], kind))
        .map_err(|error| GenerateError(error.to_string()))
}

/// The new definition, after dropping the old object when editing; both
/// run in one transaction.
fn replaced(old: Option<String>, created: Vec<String>) -> ObjectPlan {
    plan(old.into_iter().chain(created).collect(), &[])
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

/// `CREATE`/`DROP DATABASE` cannot run in a transaction.
fn on_home(sql: String, warnings: &[&str], destroys: bool) -> ObjectPlan {
    ObjectPlan {
        destructive: destroys,
        run: PlanRun::HomeDatabase,
        ..plan(vec![sql], warnings)
    }
}

fn create_table(table: &ObjectName, definition: &NewTable) -> Result<ObjectPlan, AlterError> {
    definition.validate()?;
    let relation = Relation {
        schema: &table.schema,
        table: &table.name,
    };
    let mut lines = definition
        .columns
        .iter()
        .map(|column| column_spec(&table.name, column, false))
        .collect::<Result<Vec<_>, _>>()?;
    if !definition.primary_key.is_empty() {
        lines.push(format!(
            "PRIMARY KEY ({})",
            column_list(&definition.primary_key)?
        ));
    }
    for key in &definition.foreign_keys {
        lines.push(foreign_key_spec(key)?);
    }
    for check in &definition.checks {
        lines.push(check_spec(check)?);
    }
    checked(&table.schema)?;
    let mut statements = vec![format!(
        "CREATE TABLE {} (\n  {}\n)",
        relation.quoted(),
        lines.join(",\n  ")
    )];
    for index in &definition.indexes {
        statements.push(index_statement(&relation, index)?);
    }
    let comments = std::iter::once((Level::Table, definition.comment.as_deref())).chain(
        definition
            .columns
            .iter()
            .map(|column| (Level::Column(&column.name), column.comment.as_deref())),
    );
    for (level, comment) in comments {
        statements.extend(comment_statement(&relation, (level, false), comment)?);
    }
    Ok(plan(statements, &[]))
}

/// A copy of the columns (types, nullability, defaults, computed columns,
/// the primary key) and optionally the rows. Identity columns become plain
/// columns so copied values stay as they are.
fn duplicate(
    table: &ObjectName,
    name: &str,
    with_data: bool,
    source: Option<&TableDetail>,
) -> Result<ObjectPlan, GenerateError> {
    let detail = source.ok_or_else(|| {
        GenerateError(format!("the structure of {} could not be read", table.name))
    })?;
    let from = name_of(table)?;
    let copy = in_schema(&table.schema, name)?;
    let mut lines = Vec::new();
    for column in &detail.columns {
        lines.push(copied_column(column)?);
    }
    let key: Vec<String> = detail
        .constraints
        .iter()
        .find(|constraint| constraint.kind == ConstraintKind::Primary)
        .map(|constraint| constraint.columns.clone())
        .unwrap_or_default();
    if !key.is_empty() {
        let list = column_list(&key).map_err(|error| fail(&error))?;
        lines.push(format!("PRIMARY KEY ({list})"));
    }
    let mut statements = vec![format!(
        "CREATE TABLE {copy} (\n  {}\n)",
        lines.join(",\n  ")
    )];
    if with_data {
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
    }
    Ok(ObjectPlan {
        warnings: vec![
            "Indexes, foreign keys, checks, triggers and IDENTITY numbering are not copied."
                .to_owned(),
        ],
        ..plan(statements, &[])
    })
}

fn copied_column(column: &ColumnDetail) -> Result<String, GenerateError> {
    let name = ident(&column.name)?;
    if column.generated == Generated::Stored {
        let expression = column.default.as_deref().unwrap_or("NULL");
        return Ok(format!("{name} AS {expression}"));
    }
    let null = if column.nullable { "NULL" } else { "NOT NULL" };
    let default = column
        .default
        .as_deref()
        .map(|sql| format!(" DEFAULT {sql}"))
        .unwrap_or_default();
    Ok(format!("{name} {} {null}{default}", column.data_type))
}

#[cfg(test)]
#[path = "objects_tests.rs"]
mod tests;
