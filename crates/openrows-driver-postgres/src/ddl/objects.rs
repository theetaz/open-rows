// SPDX-License-Identifier: Apache-2.0
//! Object lifecycle changes as `PostgreSQL` DDL (FR-SCHEMA-10, 11, 12).
//! Tables, views, triggers, routines and schemas change in one transaction;
//! databases are created, renamed and dropped from the `postgres` database,
//! outside a transaction.
use super::keys::{self, column_definition, column_list};
use super::{AlterError, quote, relation, text};
use openrows_driver_api::GenerateError;
use openrows_driver_api::objects::{
    NewTable, ObjectChange, ObjectName, ObjectPlan, PlanRun, RoutineKind, definition_statements,
    view_query,
};
use openrows_driver_api::schema::{Generated, TableDetail};
use openrows_sql::Dialect;
use std::fmt::Write as _;

const DIALECT: Dialect = Dialect::Postgres;
const HOME: &str = "postgres";

/// The plan for `change`; `source` is the duplicated table's detail.
///
/// # Errors
/// Invalid names, definitions and table definitions.
pub fn object_sql(
    change: &ObjectChange,
    source: Option<&TableDetail>,
) -> Result<ObjectPlan, GenerateError> {
    let fail = |error: AlterError| GenerateError(error.to_string());
    match change {
        ObjectChange::CreateTable { table, definition } => {
            create_table(table, definition).map_err(fail)
        }
        ObjectChange::RenameTable { table, name } => {
            let sql = format!("ALTER TABLE {} RENAME TO {}", name_of(table)?, ident(name)?);
            Ok(plan(
                vec![sql],
                &["Code that uses the old table name stops working."],
            ))
        }
        ObjectChange::DuplicateTable {
            table,
            name,
            with_data,
        } => duplicate(table, name, *with_data, source).map_err(fail),
        ObjectChange::TruncateTable { table, cascade } => Ok(destructive(
            vec![format!(
                "TRUNCATE TABLE {}{}",
                name_of(table)?,
                cascaded(*cascade)
            )],
            &[
                "Every row is deleted; this cannot be undone once committed.",
                "TRUNCATE takes an exclusive lock on the table.",
            ],
        )),
        ObjectChange::DropTable { table, cascade } => Ok(destructive(
            vec![format!(
                "DROP TABLE {}{}",
                name_of(table)?,
                cascaded(*cascade)
            )],
            &["The table and every row in it are deleted permanently."],
        )),
        ObjectChange::SaveView {
            view,
            query,
            materialized,
            replace,
        } => save_view(view, query, *materialized, *replace),
        ObjectChange::DropView {
            view,
            materialized,
            cascade,
        } => {
            let kind = if *materialized {
                "MATERIALIZED VIEW"
            } else {
                "VIEW"
            };
            Ok(destructive(
                vec![format!(
                    "DROP {kind} {}{}",
                    name_of(view)?,
                    cascaded(*cascade)
                )],
                &["Queries and views that use it stop working."],
            ))
        }
        other => routines_and_containers(other),
    }
}

fn routines_and_containers(change: &ObjectChange) -> Result<ObjectPlan, GenerateError> {
    match change {
        ObjectChange::SaveTrigger {
            table,
            replaces,
            definition,
        } => {
            let mut statements = Vec::new();
            if let Some(old) = replaces {
                statements.push(format!(
                    "DROP TRIGGER IF EXISTS {} ON {}",
                    ident(old)?,
                    name_of(table)?
                ));
            }
            statements.extend(checked(definition, (&["FUNCTION"], "TRIGGER"))?);
            Ok(plan(statements, &[]))
        }
        ObjectChange::DropTrigger { table, name } => Ok(destructive(
            vec![format!(
                "DROP TRIGGER {} ON {}",
                ident(name)?,
                name_of(table)?
            )],
            &["The trigger no longer runs on changes to the table."],
        )),
        ObjectChange::SaveRoutine {
            kind,
            replaces,
            definition,
            ..
        } => {
            let statements = checked(definition, (&[], kind.keyword()))?;
            let warnings: &[&str] = if replaces.is_some() {
                &["Changing the arguments creates a new overload; the old routine stays."]
            } else {
                &[]
            };
            Ok(plan(statements, warnings))
        }
        ObjectChange::DropRoutine {
            routine,
            kind,
            arguments,
        } => Ok(destructive(
            vec![drop_routine(routine, *kind, arguments.as_deref())?],
            &["Code that calls it stops working."],
        )),
        other => containers(other),
    }
}

fn containers(change: &ObjectChange) -> Result<ObjectPlan, GenerateError> {
    let busy = "Nobody may be connected to it, OpenRows included: close the database first.";
    match change {
        ObjectChange::CreateDatabase {
            name,
            encoding,
            collation,
        } => Ok(on_home(
            create_database(name, encoding.as_deref(), collation.as_deref())?,
            &[],
            false,
        )),
        ObjectChange::RenameDatabase { name, to } => {
            maintenance(name)?;
            let sql = format!("ALTER DATABASE {} RENAME TO {}", ident(name)?, ident(to)?);
            Ok(on_home(sql, &[busy], false))
        }
        ObjectChange::DropDatabase { name } => {
            maintenance(name)?;
            let sql = format!("DROP DATABASE {}", ident(name)?);
            Ok(on_home(
                sql,
                &[busy, "Every object and row in it is deleted permanently."],
                true,
            ))
        }
        ObjectChange::CreateSchema { name } => {
            Ok(plan(vec![format!("CREATE SCHEMA {}", ident(name)?)], &[]))
        }
        ObjectChange::RenameSchema { name, to } => Ok(plan(
            vec![format!(
                "ALTER SCHEMA {} RENAME TO {}",
                ident(name)?,
                ident(to)?
            )],
            &["Code that names the old schema stops working."],
        )),
        ObjectChange::DropSchema { name, cascade } => Ok(destructive(
            vec![format!(
                "DROP SCHEMA {}{}",
                ident(name)?,
                cascaded(*cascade)
            )],
            &["Dropping a schema with CASCADE deletes every object in it."],
        )),
        _ => Err(GenerateError("unsupported change".to_owned())),
    }
}

fn ident(name: &str) -> Result<String, GenerateError> {
    quote(name).map_err(|error| GenerateError(error.to_string()))
}

fn name_of(object: &ObjectName) -> Result<String, GenerateError> {
    relation(&object.schema, &object.name).map_err(|error| GenerateError(error.to_string()))
}

fn cascaded(cascade: bool) -> &'static str {
    if cascade { " CASCADE" } else { "" }
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

fn checked(definition: &str, kinds: (&[&str], &str)) -> Result<Vec<String>, GenerateError> {
    definition_statements(definition, DIALECT, kinds)
        .map_err(|error| GenerateError(error.to_string()))
}

fn maintenance(name: &str) -> Result<(), GenerateError> {
    if name == HOME {
        return Err(GenerateError(format!(
            "OpenRows runs this from the {HOME} database, so it cannot rename or drop {HOME}"
        )));
    }
    Ok(())
}

fn create_table(table: &ObjectName, definition: &NewTable) -> Result<ObjectPlan, AlterError> {
    definition.validate()?;
    let name = relation(&table.schema, &table.name)?;
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
    for key in &definition.foreign_keys {
        lines.push(keys::foreign_key(key, &table.schema)?);
    }
    for check in &definition.checks {
        lines.push(keys::check(check)?);
    }
    let mut statements = vec![format!(
        "CREATE TABLE {name} (\n    {}\n)",
        lines.join(",\n    ")
    )];
    for index in &definition.indexes {
        statements.push(keys::create_index(&name, index)?);
    }
    if let Some(comment) = &definition.comment {
        statements.push(format!(
            "COMMENT ON TABLE {name} IS {}",
            text(Some(comment))?
        ));
    }
    for column in &definition.columns {
        if let Some(comment) = &column.comment {
            statements.push(format!(
                "COMMENT ON COLUMN {name}.{} IS {}",
                quote(&column.name)?,
                text(Some(comment))?
            ));
        }
    }
    Ok(plan(statements, &[]))
}

fn duplicate(
    table: &ObjectName,
    name: &str,
    with_data: bool,
    source: Option<&TableDetail>,
) -> Result<ObjectPlan, AlterError> {
    let from = relation(&table.schema, &table.name)?;
    let copy = relation(&table.schema, name)?;
    let mut statements = vec![format!("CREATE TABLE {copy} (LIKE {from} INCLUDING ALL)")];
    let mut warnings = vec!["Foreign keys and triggers are not copied.".to_owned()];
    if with_data {
        let detail = source.ok_or_else(|| AlterError::UnreadableDefinition(table.name.clone()))?;
        statements.push(copy_rows(&from, &copy, detail)?);
        warnings.push("Copying the rows reads the whole table.".to_owned());
    }
    Ok(ObjectPlan {
        warnings,
        ..plan(statements, &[])
    })
}

/// `INSERT INTO copy (cols) [OVERRIDING SYSTEM VALUE] SELECT cols FROM source`;
/// stored generated columns are computed again.
fn copy_rows(from: &str, copy: &str, detail: &TableDetail) -> Result<String, AlterError> {
    let columns: Vec<String> = detail
        .columns
        .iter()
        .filter(|column| column.generated != Generated::Stored)
        .map(|column| column.name.clone())
        .collect();
    let list = column_list(&columns)?;
    let overriding = if detail
        .columns
        .iter()
        .any(|column| column.generated == Generated::IdentityAlways)
    {
        " OVERRIDING SYSTEM VALUE"
    } else {
        ""
    };
    Ok(format!(
        "INSERT INTO {copy} ({list}){overriding} SELECT {list} FROM {from}"
    ))
}

fn save_view(
    view: &ObjectName,
    query: &str,
    materialized: bool,
    replace: bool,
) -> Result<ObjectPlan, GenerateError> {
    let query = view_query(query, DIALECT).map_err(|error| GenerateError(error.to_string()))?;
    let name = name_of(view)?;
    if !materialized {
        let or_replace = if replace { "OR REPLACE " } else { "" };
        let warnings: &[&str] = if replace {
            &[
                "CREATE OR REPLACE VIEW fails when columns are removed, renamed or retyped; drop and create the view instead.",
            ]
        } else {
            &[]
        };
        return Ok(plan(
            vec![format!("CREATE {or_replace}VIEW {name} AS\n{query}")],
            warnings,
        ));
    }
    let create = format!("CREATE MATERIALIZED VIEW {name} AS\n{query}");
    if !replace {
        return Ok(plan(
            vec![create],
            &["The query runs now to fill the materialized view."],
        ));
    }
    Ok(destructive(
        vec![format!("DROP MATERIALIZED VIEW {name}"), create],
        &[
            "The materialized view and its indexes are dropped and filled again; objects that depend on it block this.",
        ],
    ))
}

fn drop_routine(
    routine: &ObjectName,
    kind: RoutineKind,
    arguments: Option<&str>,
) -> Result<String, GenerateError> {
    let arguments = arguments.unwrap_or_default();
    if arguments.contains([';', '\0']) {
        return Err(GenerateError(format!(
            "{arguments:?} are not routine arguments"
        )));
    }
    Ok(format!(
        "DROP {} {}({arguments})",
        kind.keyword(),
        name_of(routine)?
    ))
}

fn create_database(
    name: &str,
    encoding: Option<&str>,
    collation: Option<&str>,
) -> Result<String, GenerateError> {
    let literal = |value: &str| text(Some(value)).map_err(|error| GenerateError(error.to_string()));
    let mut sql = format!("CREATE DATABASE {}", ident(name)?);
    if let Some(encoding) = encoding.filter(|value| !value.is_empty()) {
        let _ = write!(sql, " ENCODING {}", literal(encoding)?);
    }
    if let Some(collation) = collation.filter(|value| !value.is_empty()) {
        let collation = literal(collation)?;
        let _ = write!(sql, " LC_COLLATE {collation} LC_CTYPE {collation}");
    }
    if encoding.is_some_and(|value| !value.is_empty())
        || collation.is_some_and(|value| !value.is_empty())
    {
        sql.push_str(" TEMPLATE template0");
    }
    Ok(sql)
}

#[cfg(test)]
#[path = "objects_tests.rs"]
mod tests;
