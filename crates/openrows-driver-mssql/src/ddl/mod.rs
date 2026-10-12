// SPDX-License-Identifier: Apache-2.0
//! Structure changes as T-SQL (docs/10 §3), run in one transaction (SQL
//! Server DDL is transactional). `ALTER COLUMN` restates the type and
//! nullability (and keeps or sets the column's collation); defaults are
//! named constraints, dropped and re-added; comments are `MS_Description`
//! extended properties; renames use `sp_rename`, after every other change
//! to the same objects; foreign keys and checks are added after them. A
//! column added with a default fills existing rows (`WITH VALUES`), as on
//! the other engines. `IDENTITY` cannot be added to or removed from an
//! existing column.
pub(crate) mod definition;
mod objects;
mod sql;

pub(crate) use objects::object_sql;

use openrows_driver_api::schema::{
    ColumnDetail, DefaultValue, Identity, NewColumn, NewIndex, SchemaChange, TableDetail,
};
use openrows_driver_api::structure::{
    AlterError, AlterPlan, AlterTable, ColumnEdits, Comment, IndexDrops, StagedChanges,
};

use definition::{ColumnClauses, Relation, column_clauses};
use sql::{
    Level, check_spec, checked, collation, column_spec, comment_statement, data_type, default_sql,
    foreign_key_spec, index_statement, literal, no_charset,
};

/// The DDL view: the definition the catalog rebuilt, else a note.
pub(crate) fn table_ddl((schema, table): (&str, &str), detail: &TableDetail) -> String {
    detail
        .info
        .as_ref()
        .and_then(|info| info.definition.clone())
        .unwrap_or_else(|| format!("-- {schema}.{table} was not found\n"))
}

/// Validate `changes` against the catalog and generate the statements.
///
/// # Errors
/// See [`AlterError`].
pub(crate) fn alter_sql(
    target: &AlterTable<'_>,
    changes: &[SchemaChange],
) -> Result<AlterPlan, AlterError> {
    let staged = StagedChanges::stage(target, changes, IndexDrops::NotConstraints)?;
    openrows_driver_api::structure::validate_options(
        &staged,
        openrows_sql::Dialect::SqlServer,
        changes.len(),
    )?;
    let relation = Relation {
        schema: target.schema,
        table: target.table,
    };
    let mut plan = Plan {
        relation,
        definition: target
            .detail
            .info
            .as_ref()
            .and_then(|info| info.definition.as_deref()),
        out: AlterPlan::default(),
    };
    plan.drops(&staged)?;
    plan.existing_columns(target.detail, &staged)?;
    for column in &staged.adds {
        let mut column = (*column).clone();
        if staged
            .primary_keys
            .iter()
            .any(|key| key.columns.contains(&column.name))
        {
            column.nullable = false;
        }
        plan.add_column(&column)?;
    }
    plan.renames(target.detail, &staged)?;
    for index in &staged.index_adds {
        plan.add_index(index)?;
    }
    plan.constraints(&staged)?;
    plan.table_options(target.detail, &staged)?;
    if plan.out.statements.is_empty() {
        return Err(AlterError::Empty);
    }
    Ok(plan.out)
}

struct Plan<'a> {
    relation: Relation<'a>,
    definition: Option<&'a str>,
    out: AlterPlan,
}

impl Plan<'_> {
    fn push(&mut self, statement: String) {
        self.out.statements.push(statement);
    }

    fn warn(&mut self, warning: String) {
        self.out.warnings.push(warning);
    }

    fn alter(&self) -> String {
        format!("ALTER TABLE {}", self.relation.quoted())
    }

    fn clauses(&self, column: &str) -> ColumnClauses {
        self.definition
            .and_then(|definition| column_clauses(definition, column))
            .unwrap_or_default()
    }

    fn drops(&mut self, staged: &StagedChanges<'_>) -> Result<(), AlterError> {
        for &name in &staged.constraint_drops {
            let statement = format!("{} DROP CONSTRAINT {}", self.alter(), checked(name)?);
            self.push(statement);
            self.out.destructive = true;
        }
        for &name in &staged.index_drops {
            let statement = format!(
                "DROP INDEX {} ON {}",
                checked(name)?,
                self.relation.quoted()
            );
            self.push(statement);
            self.out.destructive = true;
        }
        for &name in &staged.drops {
            self.drop_default(name)?;
            let statement = format!("{} DROP COLUMN {}", self.alter(), checked(name)?);
            self.push(statement);
            self.warn(format!(
                "Dropping column {name:?} permanently deletes its data."
            ));
            self.out.destructive = true;
        }
        Ok(())
    }

    fn drop_default(&mut self, column: &str) -> Result<(), AlterError> {
        if let Some(name) = self.clauses(column).default_constraint {
            let statement = format!("{} DROP CONSTRAINT {}", self.alter(), checked(&name)?);
            self.push(statement);
        }
        Ok(())
    }

    fn existing_columns(
        &mut self,
        detail: &TableDetail,
        staged: &StagedChanges<'_>,
    ) -> Result<(), AlterError> {
        for column in &detail.columns {
            let chosen = staged.columns.get(column.name.as_str());
            let mut edits = chosen.copied().unwrap_or_default();
            let primary = column.nullable && staged.primary_column(&column.name);
            if primary {
                edits.nullable = Some(false);
            }
            if chosen.is_some() || primary {
                self.column(column, &edits)?;
            }
        }
        Ok(())
    }

    fn column(&mut self, column: &ColumnDetail, edits: &ColumnEdits<'_>) -> Result<(), AlterError> {
        let name = column.name.as_str();
        if edits.identity.is_some() {
            return Err(AlterError::Unsupported(format!(
                "SQL Server cannot add or remove IDENTITY on the existing column {name:?}; add a \
                 new column instead"
            )));
        }
        if edits.data_type.is_some() || edits.nullable.is_some() || edits.collation.is_some() {
            self.alter_column(column, edits)?;
        }
        if let Some(default) = edits.default {
            self.drop_default(name)?;
            if let Some(sql) = default_sql(name, default)? {
                let constraint = checked(&format!("DF_{}_{name}", self.relation.table))?;
                let statement = format!(
                    "{} ADD CONSTRAINT {constraint} DEFAULT {sql} FOR {}",
                    self.alter(),
                    checked(name)?
                );
                self.push(statement);
            }
        }
        if let Some(Comment(comment)) = edits.comment {
            let level = (Level::Column(name), column.comment.is_some());
            self.comment(level, comment)?;
        }
        Ok(())
    }

    /// `ALTER COLUMN` with the new or current type and nullability.
    fn alter_column(
        &mut self,
        column: &ColumnDetail,
        edits: &ColumnEdits<'_>,
    ) -> Result<(), AlterError> {
        let name = column.name.as_str();
        let mut spec = if let Some(text) = edits.data_type {
            self.warn(format!(
                "Changing the type of {name:?} fails or truncates values that cannot be converted."
            ));
            self.out.destructive = true;
            data_type(text)?
        } else {
            column.data_type.clone()
        };
        let kept = edits
            .data_type
            .is_none()
            .then(|| self.clauses(name).collation);
        let chosen = match edits.collation {
            Some(change) => {
                no_charset(change.charset)?;
                change.collation.map(collation).transpose()?
            }
            None => kept.flatten(),
        };
        if let Some(name) = chosen {
            spec.push_str(" COLLATE ");
            spec.push_str(&name);
        }
        let nullable = edits.nullable.unwrap_or(column.nullable);
        if !nullable && edits.nullable == Some(false) {
            self.warn(format!(
                "NOT NULL on {name:?} fails if the column contains NULLs."
            ));
        }
        spec.push_str(if nullable { " NULL" } else { " NOT NULL" });
        let statement = format!("{} ALTER COLUMN {} {spec}", self.alter(), checked(name)?);
        self.push(statement);
        Ok(())
    }

    fn add_column(&mut self, column: &NewColumn) -> Result<(), AlterError> {
        let spec = column_spec(self.relation.table, column, true)?;
        let unfilled = !column.nullable
            && column.default == DefaultValue::None
            && column.identity == Identity::None;
        if unfilled {
            self.warn(format!(
                "Column {:?} is NOT NULL without a default; adding it fails if the table has rows.",
                column.name
            ));
        }
        let statement = format!("{} ADD {spec}", self.alter());
        self.push(statement);
        if let Some(comment) = column.comment.as_deref() {
            self.comment((Level::Column(&column.name), false), Some(comment))?;
        }
        Ok(())
    }

    fn renames(
        &mut self,
        detail: &TableDetail,
        staged: &StagedChanges<'_>,
    ) -> Result<(), AlterError> {
        for column in &detail.columns {
            let Some(rename) = staged
                .columns
                .get(column.name.as_str())
                .and_then(|edits| edits.renamed(&column.name))
            else {
                continue;
            };
            let object = format!("{}.{}", self.relation.quoted(), checked(&column.name)?);
            let statement = format!(
                "EXEC sys.sp_rename {}, {}, N'COLUMN'",
                literal(&object)?,
                literal(rename)?
            );
            self.push(statement);
        }
        Ok(())
    }

    fn add_index(&mut self, index: &NewIndex) -> Result<(), AlterError> {
        let statement = index_statement(&self.relation, index)?;
        self.push(statement);
        Ok(())
    }

    /// Foreign keys and checks, on the final column names.
    fn constraints(&mut self, staged: &StagedChanges<'_>) -> Result<(), AlterError> {
        for (keys, primary) in [(&staged.primary_keys, true), (&staged.unique_keys, false)] {
            for key in keys {
                let sql = openrows_driver_api::structure::key_sql(
                    key,
                    primary,
                    openrows_sql::Dialect::SqlServer,
                )?;
                self.push(format!("{} ADD {sql}", self.alter()));
                self.warn("Adding a key scans and locks the table; duplicate values or NULL primary-key values cause failure.".to_owned());
            }
        }
        for key in &staged.foreign_keys {
            let statement = format!("{} ADD {}", self.alter(), foreign_key_spec(key)?);
            self.push(statement);
        }
        for check in &staged.checks {
            let statement = format!("{} ADD {}", self.alter(), check_spec(check)?);
            self.push(statement);
            self.warn("A new check fails if existing rows break it.".to_owned());
        }
        Ok(())
    }

    fn table_options(
        &mut self,
        detail: &TableDetail,
        staged: &StagedChanges<'_>,
    ) -> Result<(), AlterError> {
        if let Some(Comment(comment)) = staged.comment {
            let exists = detail
                .info
                .as_ref()
                .is_some_and(|info| info.comment.is_some());
            self.comment((Level::Table, exists), comment)?;
        }
        if let Some(name) = staged.rename {
            let statement = format!(
                "EXEC sys.sp_rename {}, {}",
                literal(&self.relation.quoted())?,
                literal(name)?
            );
            self.push(statement);
            self.warn("Code that uses the old table name stops working.".to_owned());
        }
        Ok(())
    }

    fn comment(
        &mut self,
        level: (Level<'_>, bool),
        comment: Option<&str>,
    ) -> Result<(), AlterError> {
        if let Some(statement) = comment_statement(&self.relation, level, comment)? {
            self.push(statement);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

#[cfg(test)]
#[path = "structure_options_tests.rs"]
mod option_tests;
