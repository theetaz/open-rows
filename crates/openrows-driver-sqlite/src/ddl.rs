// SPDX-License-Identifier: Apache-2.0
//! A staged structure change set as `SQLite` DDL. `ALTER TABLE` covers
//! renames, plain added columns and dropping unconstrained columns; type,
//! nullability and default changes (and drops or adds `ALTER TABLE`
//! refuses) rebuild the table with the twelve-step procedure of
//! <https://sqlite.org/lang_altertable.html#otheralter>: create the new
//! definition, copy the rows, drop the old table, rename the new one and
//! recreate its indexes and triggers. The editor runs every statement in one
//! transaction with foreign keys off and checks them before committing.
mod constraints;
pub(crate) mod objects;
mod rebuild;

pub(crate) use constraints::table_checks;

use openrows_driver_api::schema::{
    DefaultValue, Identity, IndexMethod, NewColumn, NewIndex, SchemaChange,
};
use openrows_driver_api::structure::{
    AlterError, AlterPlan, AlterTable, IndexDrops, StagedChanges,
};
use openrows_sql::Dialect;
use openrows_sql::ddl::{ColumnDefinition, string_literal, valid_type};
use std::fmt::Write as _;

const DIALECT: Dialect = Dialect::Sqlite;

/// Validate `changes` against the catalog and generate ordered DDL.
///
/// # Errors
/// See [`AlterError`]; comments are [`AlterError::Unsupported`].
pub(crate) fn alter_sql(
    target: &AlterTable<'_>,
    changes: &[SchemaChange],
) -> Result<AlterPlan, AlterError> {
    let staged = StagedChanges::stage(target, changes, IndexDrops::NotConstraints)?;
    unsupported(&staged)?;
    let mut plan = Plan {
        target,
        staged: &staged,
        out: AlterPlan::default(),
    };
    if rebuild::needed(target.detail, &staged) {
        rebuild::write(&mut plan)?;
    } else {
        plan.alter_columns()?;
    }
    plan.renames()?;
    plan.indexes()?;
    plan.rename_table()?;
    if plan.out.statements.is_empty() {
        return Err(AlterError::Empty);
    }
    Ok(plan.out)
}

/// Comments, identity numbering and character sets, which `SQLite` lacks.
fn unsupported(staged: &StagedChanges<'_>) -> Result<(), AlterError> {
    let commented = staged.comment.is_some()
        || staged.columns.values().any(|edits| edits.comment.is_some())
        || staged.adds.iter().any(|column| column.comment.is_some());
    if commented {
        return Err(AlterError::Unsupported(
            "SQLite tables and columns have no comments".to_owned(),
        ));
    }
    let numbered = staged
        .columns
        .values()
        .any(|edits| edits.identity.is_some())
        || staged
            .adds
            .iter()
            .any(|column| column.identity != Identity::None);
    if numbered {
        return Err(AlterError::Unsupported(NUMBERING.to_owned()));
    }
    let charset = staged
        .columns
        .values()
        .any(|edits| edits.collation.is_some_and(|c| c.charset.is_some()))
        || staged.adds.iter().any(|column| column.charset.is_some());
    if charset {
        return Err(AlterError::Unsupported(
            "SQLite columns have no character set".to_owned(),
        ));
    }
    Ok(())
}

/// Why identity numbering is refused.
const NUMBERING: &str = "SQLite numbers the rows of an INTEGER PRIMARY KEY column by itself";

/// A collation name: letters, digits and `_` (`BINARY`, `NOCASE`, `RTRIM`).
fn collation_name(name: Option<&str>) -> Result<Option<&str>, AlterError> {
    match name.map(str::trim).filter(|name| !name.is_empty()) {
        Some(name) if name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') => {
            Ok(Some(name))
        }
        Some(name) => Err(AlterError::InvalidType(name.to_owned())),
        None => Ok(None),
    }
}

fn quote(name: &str) -> Result<String, AlterError> {
    if name.is_empty() {
        return Err(AlterError::InvalidName);
    }
    DIALECT
        .quote_ident(name)
        .map_err(|_| AlterError::InvalidName)
}

fn relation(schema: &str, name: &str) -> Result<String, AlterError> {
    Ok(format!("{}.{}", quote(schema)?, quote(name)?))
}

fn data_type(text: &str) -> Result<String, AlterError> {
    let trimmed = text.trim();
    if valid_type(DIALECT, trimmed) {
        Ok(trimmed.to_owned())
    } else {
        Err(AlterError::InvalidType(text.to_owned()))
    }
}

/// `None` for no default, else the SQL after `DEFAULT`.
fn default_sql(column: &str, default: &DefaultValue) -> Result<Option<String>, AlterError> {
    match default {
        DefaultValue::None => Ok(None),
        DefaultValue::Literal(value) => string_literal(DIALECT, value)
            .map(Some)
            .ok_or(AlterError::InvalidName),
        DefaultValue::Expression(sql) => {
            let sql = sql.trim();
            if sql.is_empty() || sql.contains('\0') {
                Err(AlterError::InvalidDefault(column.to_owned()))
            } else {
                Ok(Some(format!("({sql})")))
            }
        }
    }
}

/// `name type [COLLATE …] [NOT NULL] [DEFAULT …]` of a column to add.
fn new_column(column: &NewColumn) -> Result<String, AlterError> {
    let mut definition =
        ColumnDefinition::new(&quote(&column.name)?, &data_type(&column.data_type)?);
    definition.set_collation(None, collation_name(column.collation.as_deref())?);
    definition.set_nullable(column.nullable);
    definition.set_default(default_sql(&column.name, &column.default)?.as_deref());
    Ok(definition.render())
}

/// The plan being written for one table.
pub(crate) struct Plan<'a> {
    target: &'a AlterTable<'a>,
    staged: &'a StagedChanges<'a>,
    out: AlterPlan,
}

impl Plan<'_> {
    fn push(&mut self, sql: String) {
        self.out.statements.push(sql);
    }

    fn warn(&mut self, warning: String) {
        self.out.warnings.push(warning);
    }

    fn relation(&self) -> Result<String, AlterError> {
        relation(self.target.schema, self.target.table)
    }

    /// Drops and adds that `ALTER TABLE` can do itself.
    fn alter_columns(&mut self) -> Result<(), AlterError> {
        let relation = self.relation()?;
        for &name in &self.staged.drops {
            self.push(format!(
                "ALTER TABLE {relation} DROP COLUMN {}",
                quote(name)?
            ));
            self.warn(format!(
                "Dropping column {name:?} permanently deletes its data."
            ));
            self.out.destructive = true;
        }
        for column in &self.staged.adds {
            let definition = new_column(column)?;
            self.push(format!("ALTER TABLE {relation} ADD COLUMN {definition}"));
        }
        Ok(())
    }

    fn renames(&mut self) -> Result<(), AlterError> {
        let relation = self.relation()?;
        for (&column, edits) in &self.staged.columns {
            if let Some(name) = edits.renamed(column) {
                self.push(format!(
                    "ALTER TABLE {relation} RENAME COLUMN {} TO {}",
                    quote(column)?,
                    quote(name)?
                ));
            }
        }
        Ok(())
    }

    /// Index drops (a rebuild already left them out) and creates.
    fn indexes(&mut self) -> Result<(), AlterError> {
        let rebuilt = rebuild::needed(self.target.detail, self.staged);
        for &name in &self.staged.index_drops {
            if !rebuilt {
                self.push(format!(
                    "DROP INDEX {}",
                    relation(self.target.schema, name)?
                ));
            }
            self.warn(format!(
                "Dropping index {name:?} can slow down queries that use it."
            ));
            self.out.destructive = true;
        }
        for index in &self.staged.index_adds {
            let sql = self.create_index(index)?;
            self.push(sql);
        }
        Ok(())
    }

    fn create_index(&self, index: &NewIndex) -> Result<String, AlterError> {
        if index.method != IndexMethod::Btree {
            return Err(AlterError::Unsupported(
                "SQLite indexes are always B-trees".to_owned(),
            ));
        }
        let unique = if index.unique { "UNIQUE " } else { "" };
        let name = match index.name.as_deref().filter(|name| !name.is_empty()) {
            Some(name) => name.to_owned(),
            None => format!("{}_{}_idx", self.target.table, index.columns.join("_")),
        };
        let columns = index
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
            "CREATE {unique}INDEX {} ON {} ({columns})",
            relation(self.target.schema, &name)?,
            quote(self.target.table)?
        );
        if let Some(predicate) = index.predicate.as_deref().map(str::trim) {
            let _ = write!(sql, " WHERE ({predicate})");
        }
        Ok(sql)
    }

    fn rename_table(&mut self) -> Result<(), AlterError> {
        if let Some(name) = self.staged.rename {
            let relation = self.relation()?;
            self.push(format!("ALTER TABLE {relation} RENAME TO {}", quote(name)?));
            self.warn("Code that uses the old table name stops working.".to_owned());
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "ddl_tests.rs"]
mod tests;
