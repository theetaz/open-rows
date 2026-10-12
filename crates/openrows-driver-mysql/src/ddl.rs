// SPDX-License-Identifier: Apache-2.0
//! A staged structure change set as one `ALTER TABLE` statement (`MySQL`
//! 8 applies it atomically). Changed columns are rewritten from the
//! server's own `SHOW CREATE TABLE` definition, so character set,
//! collation, `AUTO_INCREMENT`, `ON UPDATE` and generation clauses survive a
//! `MODIFY`; a pure rename is `RENAME COLUMN`. [`objects`] generates the
//! object lifecycle changes.
mod keys;
pub(crate) mod objects;

use keys::{auto_increment, charset_name, column_definition};
use openrows_driver_api::schema::{
    ColumnDetail, ConstraintKind, DefaultValue, Identity, NewColumn, SchemaChange,
};
use openrows_driver_api::structure::{
    AlterError, AlterPlan, AlterTable, ColumnEdits, Comment, IndexDrops, StagedChanges,
    narrowing_warning,
};
use openrows_sql::Dialect;
use openrows_sql::ddl::{ColumnDefinition, TableDefinition, string_literal, valid_type};

const DIALECT: Dialect = Dialect::MySql;

/// Validate `changes` against the catalog and generate the statement.
///
/// # Errors
/// See [`AlterError`].
pub(crate) fn alter_sql(
    target: &AlterTable<'_>,
    changes: &[SchemaChange],
) -> Result<AlterPlan, AlterError> {
    let staged = StagedChanges::stage(target, changes, IndexDrops::NotPrimary)?;
    openrows_driver_api::structure::validate_options(&staged, Dialect::MySql, changes.len())?;
    let definition = target
        .detail
        .info
        .as_ref()
        .and_then(|info| info.definition.as_deref())
        .and_then(|sql| TableDefinition::parse(sql, DIALECT));
    let mut plan = Plan {
        table: target.table,
        definition,
        specs: Vec::new(),
        out: AlterPlan {
            run: openrows_driver_api::structure::AlterRun::Autocommit,
            ..AlterPlan::default()
        },
    };
    plan.constraint_drops(target, &staged)?;
    plan.columns(target, &staged)?;
    plan.reorders(target, &staged)?;
    plan.indexes(&staged)?;
    plan.constraints(target.schema, &staged)?;
    plan.table_options(&staged)?;
    if !staged.primary_keys.is_empty() {
        plan.specs.push("ALGORITHM=INPLACE".into());
        plan.out.run = openrows_driver_api::structure::AlterRun::StrictMySql;
        plan.warn("Primary-key changes use ALGORITHM=INPLACE to prevent NULLs being replaced with defaults. They require a supporting storage engine and strict SQL mode; incompatible changes must be staged separately.".into());
    }
    if plan.specs.is_empty() {
        return Err(AlterError::Empty);
    }
    let relation = DIALECT
        .qualified(target.schema, target.table)
        .map_err(|_| AlterError::InvalidName)?;
    plan.out.statements = vec![format!(
        "ALTER TABLE {relation}\n  {}",
        plan.specs.join(",\n  ")
    )];
    plan.out
        .warnings
        .push("MySQL commits ALTER TABLE at once; it cannot be rolled back.".to_owned());
    Ok(plan.out)
}

pub(crate) fn quote(name: &str) -> Result<String, AlterError> {
    if name.is_empty() {
        return Err(AlterError::InvalidName);
    }
    DIALECT
        .quote_ident(name)
        .map_err(|_| AlterError::InvalidName)
}

fn literal(text: &str) -> Result<String, AlterError> {
    string_literal(DIALECT, text).ok_or(AlterError::InvalidName)
}

fn data_type(text: &str) -> Result<String, AlterError> {
    let trimmed = text.trim();
    if valid_type(DIALECT, trimmed) {
        Ok(trimmed.to_owned())
    } else {
        Err(AlterError::InvalidType(text.to_owned()))
    }
}

/// `None` for no default, else the SQL after `DEFAULT`; expressions are
/// parenthesised (`MySQL` 8.0.13+ expression defaults).
fn default_sql(column: &str, default: &DefaultValue) -> Result<Option<String>, AlterError> {
    match default {
        DefaultValue::None => Ok(None),
        DefaultValue::Literal(value) => literal(value).map(Some),
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

struct Plan<'a> {
    table: &'a str,
    definition: Option<TableDefinition>,
    specs: Vec<String>,
    out: AlterPlan,
}

impl Plan<'_> {
    fn warn(&mut self, warning: String) {
        self.out.warnings.push(warning);
    }

    /// `DROP FOREIGN KEY`, `DROP CHECK`-style `DROP CONSTRAINT`, `DROP
    /// INDEX` of a unique key, `DROP PRIMARY KEY`.
    fn constraint_drops(
        &mut self,
        target: &AlterTable<'_>,
        staged: &StagedChanges<'_>,
    ) -> Result<(), AlterError> {
        for &name in &staged.constraint_drops {
            let kind = target
                .detail
                .constraints
                .iter()
                .find(|constraint| constraint.name == name)
                .map(|constraint| constraint.kind);
            let spec = match kind {
                Some(ConstraintKind::Foreign) => format!("DROP FOREIGN KEY {}", quote(name)?),
                Some(ConstraintKind::Primary) => "DROP PRIMARY KEY".to_owned(),
                Some(ConstraintKind::Unique) => format!("DROP INDEX {}", quote(name)?),
                _ => format!("DROP CONSTRAINT {}", quote(name)?),
            };
            self.specs.push(spec);
            self.warn(format!(
                "Dropping constraint {name:?} stops the database from enforcing it."
            ));
        }
        Ok(())
    }

    fn columns(
        &mut self,
        target: &AlterTable<'_>,
        staged: &StagedChanges<'_>,
    ) -> Result<(), AlterError> {
        for &name in &staged.drops {
            self.specs.push(format!("DROP COLUMN {}", quote(name)?));
            self.warn(format!(
                "Dropping column {name:?} permanently deletes its data."
            ));
            self.out.destructive = true;
        }
        for column in &target.detail.columns {
            if staged.reorders.iter().any(|(name, _)| *name == column.name) {
                continue;
            }
            if let Some(edits) = staged.columns.get(column.name.as_str()) {
                self.column(column, edits)?;
            }
        }
        for column in &staged.adds {
            self.add_column(column)?;
        }
        Ok(())
    }

    fn column(&mut self, column: &ColumnDetail, edits: &ColumnEdits<'_>) -> Result<(), AlterError> {
        let name = column.name.as_str();
        let rename = edits.renamed(name);
        let redefined = edits.data_type.is_some()
            || edits.nullable.is_some()
            || edits.default.is_some()
            || edits.comment.is_some()
            || edits.identity.is_some()
            || edits.collation.is_some();
        if !redefined {
            if let Some(rename) = rename {
                self.specs.push(format!(
                    "RENAME COLUMN {} TO {}",
                    quote(name)?,
                    quote(rename)?
                ));
            }
            return Ok(());
        }
        let mut definition = self.current_definition(name)?;
        self.redefine(column, edits, &mut definition)?;
        let spec = match rename {
            Some(rename) => {
                definition.set_name(&quote(rename)?);
                format!("CHANGE COLUMN {} {}", quote(name)?, definition.render())
            }
            None => format!("MODIFY COLUMN {}", definition.render()),
        };
        self.specs.push(spec);
        Ok(())
    }

    /// The column's clause list from `SHOW CREATE TABLE`.
    fn current_definition(&self, name: &str) -> Result<ColumnDefinition, AlterError> {
        self.definition
            .as_ref()
            .and_then(|table| table.column(name))
            .and_then(|element| ColumnDefinition::parse(&element.text, DIALECT))
            .ok_or_else(|| AlterError::UnreadableDefinition(self.table.to_owned()))
    }

    fn redefine(
        &mut self,
        column: &ColumnDetail,
        edits: &ColumnEdits<'_>,
        definition: &mut ColumnDefinition,
    ) -> Result<(), AlterError> {
        let name = column.name.as_str();
        if let Some(text) = edits.data_type {
            let new_type = data_type(text)?;
            definition.set_type(&new_type);
            self.warn(format!(
                "Changing the type of {name:?} rebuilds the table (ALGORITHM=COPY), blocking writes, and fails or truncates values that cannot be converted."
            ));
            self.out.destructive = true;
            if let Some(warning) = narrowing_warning(name, &column.data_type, &new_type) {
                self.warn(warning);
            }
        }
        if let Some(collation) = edits.collation {
            definition.set_collation(
                charset_name(collation.charset)?,
                charset_name(collation.collation)?,
            );
            self.warn(format!(
                "Changing the character set or collation of {name:?} rebuilds the table (ALGORITHM=COPY)."
            ));
        }
        if let Some(nullable) = edits.nullable {
            definition.set_nullable(nullable);
            if !nullable {
                self.warn(format!(
                    "NOT NULL on {name:?} fails if the column contains NULLs."
                ));
            }
        }
        if let Some(default) = edits.default {
            definition.set_default(default_sql(name, default)?.as_deref());
        }
        if let Some(identity) = edits.identity {
            definition.set_auto_increment(auto_increment(identity)?);
            if identity == Identity::AutoIncrement {
                self.warn(format!(
                    "AUTO_INCREMENT on {name:?} needs the column to be indexed, usually as the primary key."
                ));
            }
        }
        if let Some(Comment(comment)) = edits.comment {
            let literal = comment.map(literal).transpose()?;
            definition.set_comment(literal.as_deref());
        }
        Ok(())
    }

    fn add_column(&mut self, column: &NewColumn) -> Result<(), AlterError> {
        if column.default == DefaultValue::None
            && !column.nullable
            && column.identity == Identity::None
        {
            self.warn(format!(
                "Column {:?} is NOT NULL without a default; existing rows get the type's zero value.",
                column.name
            ));
        }
        self.specs
            .push(format!("ADD COLUMN {}", column_definition(column)?));
        Ok(())
    }

    fn reorders(
        &mut self,
        target: &AlterTable<'_>,
        staged: &StagedChanges<'_>,
    ) -> Result<(), AlterError> {
        let mut moved = std::collections::BTreeSet::new();
        for &(name, after) in &staged.reorders {
            let column = target
                .detail
                .columns
                .iter()
                .find(|c| c.name == name)
                .ok_or_else(|| AlterError::UnknownColumn(name.into()))?;
            let mut definition = self.current_definition(name)?;
            let edits = staged.columns.get(name);
            if let Some(edits) = edits {
                self.redefine(column, edits, &mut definition)?;
            }
            let rename = edits.and_then(|e| e.renamed(name));
            definition.set_name(&quote(rename.unwrap_or(name))?);
            let position = column_position(staged, &moved, after)?;
            let action = if rename.is_some() {
                format!("CHANGE COLUMN {}", quote(name)?)
            } else {
                "MODIFY COLUMN".into()
            };
            self.specs
                .push(format!("{action} {} {position}", definition.render()));
            moved.insert(name);
        }
        if !staged.reorders.is_empty() {
            self.warn("Reordering columns may copy and lock the table; applications using positional inserts or SELECT * must be updated.".into());
        }
        Ok(())
    }

    fn indexes(&mut self, staged: &StagedChanges<'_>) -> Result<(), AlterError> {
        for &name in &staged.index_drops {
            self.specs.push(format!("DROP INDEX {}", quote(name)?));
            self.warn(format!(
                "Dropping index {name:?} can slow down queries that use it."
            ));
            self.out.destructive = true;
        }
        for index in &staged.index_adds {
            let spec = keys::index_spec(index)?;
            self.specs.push(format!("ADD {spec}"));
        }
        Ok(())
    }

    fn constraints(&mut self, schema: &str, staged: &StagedChanges<'_>) -> Result<(), AlterError> {
        for (keys, primary) in [(&staged.primary_keys, true), (&staged.unique_keys, false)] {
            for key in keys {
                let sql = openrows_driver_api::structure::key_sql(key, primary, Dialect::MySql)?;
                self.specs.push(format!("ADD {sql}"));
                self.warn("Adding a key scans and locks the table; duplicate values or NULL primary-key values cause failure.".to_owned());
            }
        }
        for key in &staged.foreign_keys {
            self.specs
                .push(format!("ADD {}", keys::foreign_key(key, schema)?));
            self.warn(format!(
                "Adding a foreign key to {} copies the table and fails if existing rows have no match.",
                key.target.table
            ));
        }
        for check in &staged.checks {
            self.specs.push(format!("ADD {}", keys::check(check)?));
            self.warn(
                "Adding a check fails if existing rows violate it (MySQL 8.0.16 or later enforces checks)."
                    .to_owned(),
            );
        }
        Ok(())
    }

    fn table_options(&mut self, staged: &StagedChanges<'_>) -> Result<(), AlterError> {
        if let Some(Comment(comment)) = staged.comment {
            self.specs.push(format!(
                "COMMENT = {}",
                literal(comment.unwrap_or_default())?
            ));
        }
        if let Some(name) = staged.rename {
            self.specs.push(format!("RENAME TO {}", quote(name)?));
            self.warn("Code that uses the old table name stops working.".to_owned());
        }
        Ok(())
    }
}

fn column_position(
    staged: &StagedChanges<'_>,
    moved: &std::collections::BTreeSet<&str>,
    after: Option<&str>,
) -> Result<String, AlterError> {
    let Some(anchor) = after else {
        return Ok("FIRST".into());
    };
    let pending =
        staged.reorders.iter().any(|(name, _)| *name == anchor) && !moved.contains(anchor);
    let chosen = if pending {
        anchor
    } else {
        staged
            .columns
            .get(anchor)
            .and_then(|edits| edits.rename)
            .unwrap_or(anchor)
    };
    Ok(format!("AFTER {}", quote(chosen)?))
}

#[cfg(test)]
#[path = "ddl_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "structure_options_tests.rs"]
mod option_tests;
