// SPDX-License-Identifier: Apache-2.0
//! Object lifecycle changes (FR-SCHEMA-10, 11, 12): create, rename,
//! duplicate, truncate and drop tables; save and drop views, triggers and
//! routines; create, rename and drop databases and schemas. A driver turns
//! one typed [`ObjectChange`] into an [`ObjectPlan`] with
//! [`crate::Driver::object_sql`]; the app reviews, confirms and runs it.
//!
//! View queries and trigger and routine definitions are SQL written by the
//! user; [`definition_statements`] checks that a definition creates exactly
//! the expected kind of object, so a definition cannot smuggle in other
//! statements.
use openrows_sql::ddl::created_kind;
use openrows_sql::{Dialect, StatementKind, classify, split};

use crate::schema::{NewCheck, NewColumn, NewForeignKey, NewIndex};

mod table;

/// A schema-qualified object name.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ObjectName {
    /// Schema (the database itself for `MySQL`, `main` for `SQLite`).
    pub schema: String,
    /// Object name.
    pub name: String,
}

/// The definition of a table to create.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NewTable {
    /// Columns in order.
    pub columns: Vec<NewColumn>,
    /// Primary-key columns in key order; empty for none.
    pub primary_key: Vec<String>,
    /// Indexes created after the table.
    pub indexes: Vec<NewIndex>,
    /// Foreign keys.
    pub foreign_keys: Vec<NewForeignKey>,
    /// Check constraints.
    pub checks: Vec<NewCheck>,
    /// Table comment.
    pub comment: Option<String>,
}

/// A function or a procedure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoutineKind {
    /// `FUNCTION`.
    Function,
    /// `PROCEDURE`.
    Procedure,
}

impl RoutineKind {
    /// `FUNCTION` or `PROCEDURE`.
    #[must_use]
    pub fn keyword(self) -> &'static str {
        match self {
            Self::Function => "FUNCTION",
            Self::Procedure => "PROCEDURE",
        }
    }
}

/// One lifecycle change of a database object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ObjectChange {
    /// `CREATE TABLE` with its columns, keys, indexes and comment.
    CreateTable {
        /// The new table.
        table: ObjectName,
        /// Its definition.
        definition: NewTable,
    },
    /// Rename a table in its schema.
    RenameTable {
        /// The table.
        table: ObjectName,
        /// New name.
        name: String,
    },
    /// A new table with the same structure, optionally with the rows.
    DuplicateTable {
        /// The source table.
        table: ObjectName,
        /// Name of the copy, in the same schema.
        name: String,
        /// Copy the rows too.
        with_data: bool,
    },
    /// Remove every row.
    TruncateTable {
        /// The table.
        table: ObjectName,
        /// Also truncate referencing tables (`PostgreSQL` `CASCADE`).
        cascade: bool,
    },
    /// `DROP TABLE`.
    DropTable {
        /// The table.
        table: ObjectName,
        /// Also drop dependent objects (`PostgreSQL` `CASCADE`).
        cascade: bool,
    },
    /// Create a view, or replace an existing one's query.
    SaveView {
        /// The view.
        view: ObjectName,
        /// The `SELECT` it shows.
        query: String,
        /// A materialized view (`PostgreSQL`).
        materialized: bool,
        /// The view exists and is replaced.
        replace: bool,
    },
    /// `DROP VIEW` / `DROP MATERIALIZED VIEW`.
    DropView {
        /// The view.
        view: ObjectName,
        /// A materialized view.
        materialized: bool,
        /// Also drop dependent objects (`PostgreSQL`).
        cascade: bool,
    },
    /// Create a trigger from its definition, replacing `replaces` if set.
    SaveTrigger {
        /// The table it fires on.
        table: ObjectName,
        /// The trigger it replaces (edit), by name.
        replaces: Option<String>,
        /// `CREATE TRIGGER …`, after any trigger functions it needs (`PostgreSQL`).
        definition: String,
    },
    /// `DROP TRIGGER`.
    DropTrigger {
        /// The table it fires on.
        table: ObjectName,
        /// Trigger name.
        name: String,
    },
    /// Create or replace a routine from its definition.
    SaveRoutine {
        /// Schema and name of the routine being edited (or the schema of a new one).
        routine: ObjectName,
        /// Function or procedure.
        kind: RoutineKind,
        /// The identity arguments of the routine it replaces (edit), e.g. `integer, text`.
        replaces: Option<String>,
        /// `CREATE [OR REPLACE] FUNCTION|PROCEDURE …`.
        definition: String,
    },
    /// `DROP FUNCTION` / `DROP PROCEDURE`.
    DropRoutine {
        /// The routine.
        routine: ObjectName,
        /// Function or procedure.
        kind: RoutineKind,
        /// Identity arguments, e.g. `integer, text` (`PostgreSQL` overloads).
        arguments: Option<String>,
    },
    /// `CREATE DATABASE` with an optional encoding and collation.
    CreateDatabase {
        /// Database name.
        name: String,
        /// Encoding (`PostgreSQL`) or character set (`MySQL`).
        encoding: Option<String>,
        /// Collation.
        collation: Option<String>,
    },
    /// Rename a database.
    RenameDatabase {
        /// Current name.
        name: String,
        /// New name.
        to: String,
    },
    /// `DROP DATABASE`.
    DropDatabase {
        /// Database name.
        name: String,
    },
    /// `CREATE SCHEMA`.
    CreateSchema {
        /// Schema name.
        name: String,
    },
    /// Rename a schema.
    RenameSchema {
        /// Current name.
        name: String,
        /// New name.
        to: String,
    },
    /// `DROP SCHEMA`.
    DropSchema {
        /// Schema name.
        name: String,
        /// Also drop everything in it.
        cascade: bool,
    },
}

/// Where and how an [`ObjectPlan`] runs.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PlanRun {
    /// All statements in one transaction where the engine allows it.
    #[default]
    Transaction,
    /// One by one, outside a transaction, on the server's home database
    /// (`CREATE DATABASE` and friends cannot run in a transaction, nor on
    /// the database they change).
    HomeDatabase,
}

/// Ordered statements, user-facing warnings and whether data can be lost.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ObjectPlan {
    /// Statements to run, without trailing `;`.
    pub statements: Vec<String>,
    /// Lock, rewrite, dependency and failure warnings for the review.
    pub warnings: Vec<String>,
    /// Drops or empties an object, or replaces a materialized view.
    pub destructive: bool,
    /// Where and how the statements run.
    pub run: PlanRun,
}

/// Why a definition is refused.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum DefinitionError {
    /// No statement at all.
    #[error("the definition is empty")]
    Empty,
    /// A statement that does not create the expected kind of object.
    #[error("statement {index} must be {expected}")]
    Unexpected {
        /// 1-based statement number.
        index: usize,
        /// What was expected, e.g. `CREATE TRIGGER`.
        expected: String,
    },
    /// A view query that is not a single read-only statement.
    #[error("a view query must be one SELECT statement")]
    NotAQuery,
}

/// The statements of a trigger or routine definition, checked: each creates
/// one of `allowed` (e.g. `FUNCTION`) and the last one creates `last`
/// (e.g. `TRIGGER`); exactly one statement creates `last`.
///
/// # Errors
/// [`DefinitionError`] for empty definitions and unexpected statements.
pub fn definition_statements(
    definition: &str,
    dialect: Dialect,
    (allowed, last): (&[&str], &str),
) -> Result<Vec<String>, DefinitionError> {
    let statements: Vec<String> = split(definition, dialect)
        .iter()
        .map(|statement| statement.text(definition).trim().to_owned())
        .filter(|text| !text.is_empty())
        .collect();
    if statements.is_empty() {
        return Err(DefinitionError::Empty);
    }
    let final_index = statements.len() - 1;
    let creates = |index: usize, kinds: &[&str]| {
        statements.get(index).is_some_and(|statement| {
            created_kind(statement, dialect).is_some_and(|kind| kinds.contains(&kind.as_str()))
        })
    };
    if !creates(final_index, &[last]) {
        return Err(DefinitionError::Unexpected {
            index: final_index + 1,
            expected: format!("CREATE {last}"),
        });
    }
    if let Some(index) = (0..final_index).find(|index| !creates(*index, allowed)) {
        let expected = allowed
            .iter()
            .map(|kind| format!("CREATE {kind}"))
            .collect::<Vec<_>>()
            .join(" or ");
        return Err(DefinitionError::Unexpected {
            index: index + 1,
            expected: if expected.is_empty() {
                "the only statement".to_owned()
            } else {
                expected
            },
        });
    }
    Ok(statements)
}

/// A view query, trimmed of white space and trailing `;`.
///
/// # Errors
/// [`DefinitionError::NotAQuery`] unless it is one read-only statement.
pub fn view_query(query: &str, dialect: Dialect) -> Result<String, DefinitionError> {
    let statements = split(query, dialect);
    let [statement] = statements.as_slice() else {
        return Err(DefinitionError::NotAQuery);
    };
    let text = statement.text(query).trim();
    let class = classify(text, dialect);
    if class.kind != StatementKind::Read || class.dangerous || text.contains('\0') {
        return Err(DefinitionError::NotAQuery);
    }
    Ok(text.to_owned())
}

#[cfg(test)]
#[path = "objects_tests.rs"]
mod tests;
