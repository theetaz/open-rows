// SPDX-License-Identifier: Apache-2.0
//! Ordered structure DDL, transactional except for concurrent index builds.
//!
//! Columns are named by their current catalog name. Statements run in this
//! order so each refers to names that exist at that point: drop
//! constraints; drop columns; per existing column (catalog order) type and
//! collation, nullability, default, identity, comment and finally the
//! rename; add columns; drop then create indexes (on final column names);
//! add foreign keys and checks; the table comment; the table rename last.
use super::keys::{self, column_definition};
use super::{data_type, default_sql, quote, relation, text};
use openrows_driver_api::schema::{
    ColumnDetail, DefaultValue, Generated, Identity, NewColumn, SchemaChange,
};
pub use openrows_driver_api::structure::{AlterError, AlterPlan, AlterTable};
use openrows_driver_api::structure::{
    ColumnEdits, Comment, IndexDrops, StagedChanges, narrowing_warning,
};

/// Validate `changes` against the catalog and generate ordered DDL.
///
/// # Errors
/// See [`AlterError`].
pub fn alter_sql(
    target: &AlterTable<'_>,
    changes: &[SchemaChange],
) -> Result<AlterPlan, AlterError> {
    let staged = StagedChanges::stage(target, changes, IndexDrops::NotConstraints)?;
    openrows_driver_api::structure::validate_options(
        &staged,
        openrows_sql::Dialect::Postgres,
        changes.len(),
    )?;
    if !staged.concurrent_indexes.is_empty()
        && target
            .detail
            .info
            .as_ref()
            .is_some_and(|i| i.kind == openrows_driver_api::schema::RelationKind::PartitionedTable)
    {
        return Err(AlterError::Unsupported("PostgreSQL cannot build an index concurrently on a partitioned parent; build indexes on individual partitions instead.".into()));
    }
    let mut plan = Plan {
        relation: relation(target.schema, target.table)?,
        schema: target.schema,
        out: AlterPlan::default(),
    };
    plan.constraint_drops(&staged)?;
    plan.columns(&target.detail.columns, &staged)?;
    plan.indexes(&staged)?;
    plan.constraints(&staged)?;
    plan.table(&staged)?;
    if plan.out.statements.is_empty() {
        return Err(AlterError::Empty);
    }
    Ok(plan.out)
}

struct Plan<'a> {
    relation: String,
    schema: &'a str,
    out: AlterPlan,
}

impl Plan<'_> {
    fn alter(&mut self, action: &str) {
        self.out
            .statements
            .push(format!("ALTER TABLE {} {action}", self.relation));
    }

    fn warn(&mut self, warning: String) {
        self.out.warnings.push(warning);
    }

    fn constraint_drops(&mut self, staged: &StagedChanges<'_>) -> Result<(), AlterError> {
        for &name in &staged.constraint_drops {
            self.alter(&format!("DROP CONSTRAINT {}", quote(name)?));
            self.warn(format!(
                "Dropping constraint {name:?} stops the database from enforcing it."
            ));
        }
        Ok(())
    }

    fn columns(
        &mut self,
        columns: &[ColumnDetail],
        staged: &StagedChanges<'_>,
    ) -> Result<(), AlterError> {
        for &name in &staged.drops {
            self.alter(&format!("DROP COLUMN {}", quote(name)?));
            self.warn(format!(
                "Dropping column {name:?} permanently deletes its data."
            ));
            self.out.destructive = true;
        }
        for column in columns {
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
        let quoted = quote(name)?;
        self.retype(column, edits)?;
        match edits.nullable {
            Some(true) => self.alter(&format!("ALTER COLUMN {quoted} DROP NOT NULL")),
            Some(false) => {
                self.alter(&format!("ALTER COLUMN {quoted} SET NOT NULL"));
                self.warn(format!(
                    "SET NOT NULL on {name:?} fails if the column contains NULLs."
                ));
            }
            None => {}
        }
        if let Some(default) = edits.default {
            match default_sql(name, default)? {
                Some(sql) => self.alter(&format!("ALTER COLUMN {quoted} SET DEFAULT {sql}")),
                None => self.alter(&format!("ALTER COLUMN {quoted} DROP DEFAULT")),
            }
        }
        if let Some(identity) = edits.identity {
            self.identity(column, &quoted, identity)?;
        }
        if let Some(Comment(comment)) = edits.comment {
            self.comment_column(&quoted, comment)?;
        }
        if let Some(rename) = edits.renamed(name) {
            self.alter(&format!("RENAME COLUMN {quoted} TO {}", quote(rename)?));
        }
        Ok(())
    }

    /// `ALTER COLUMN … TYPE … [COLLATE …]` for a type or collation change.
    fn retype(&mut self, column: &ColumnDetail, edits: &ColumnEdits<'_>) -> Result<(), AlterError> {
        if edits.data_type.is_none() && edits.collation.is_none() {
            return Ok(());
        }
        let name = column.name.as_str();
        let new_type = data_type(edits.data_type.unwrap_or(&column.data_type))?;
        let mut action = format!("ALTER COLUMN {} TYPE {new_type}", quote(name)?);
        if let Some(collation) = edits.collation {
            action.push_str(&keys::collate(collation.charset, collation.collation)?);
        }
        if let Some(expression) = edits.using_expression {
            action.push_str(" USING (");
            action.push_str(expression);
            action.push(')');
        }
        self.alter(&action);
        if edits.data_type.is_some() {
            self.warn(format!(
                "Changing the type of {name:?} rewrites the table under an exclusive lock and fails if values cannot be cast."
            ));
            self.out.destructive = true;
            if let Some(warning) = narrowing_warning(name, &column.data_type, &new_type) {
                self.warn(warning);
            }
        }
        Ok(())
    }

    fn identity(
        &mut self,
        column: &ColumnDetail,
        quoted: &str,
        identity: Identity,
    ) -> Result<(), AlterError> {
        let current = matches!(
            column.generated,
            Generated::IdentityAlways | Generated::IdentityByDefault
        );
        match (keys::identity(identity)?, current) {
            (None, true) => self.alter(&format!("ALTER COLUMN {quoted} DROP IDENTITY")),
            (None, false) => {}
            (Some(clause), true) => {
                let kind = clause.trim_end_matches(" AS IDENTITY");
                self.alter(&format!("ALTER COLUMN {quoted} SET {kind}"));
            }
            (Some(clause), false) => {
                self.alter(&format!("ALTER COLUMN {quoted} ADD {clause}"));
                self.warn(format!(
                    "The identity of {:?} needs a NOT NULL column and starts at 1; existing values may collide.",
                    column.name
                ));
            }
        }
        Ok(())
    }

    fn comment_column(&mut self, quoted: &str, comment: Option<&str>) -> Result<(), AlterError> {
        self.out.statements.push(format!(
            "COMMENT ON COLUMN {}.{quoted} IS {}",
            self.relation,
            text(comment)?
        ));
        Ok(())
    }

    fn add_column(&mut self, column: &NewColumn) -> Result<(), AlterError> {
        let definition = column_definition(column)?;
        let numbered = column.identity != Identity::None;
        if !column.nullable && !numbered && column.default == DefaultValue::None {
            self.warn(format!(
                "Adding NOT NULL column {:?} without a default fails if the table has rows.",
                column.name
            ));
        }
        self.alter(&format!("ADD COLUMN {definition}"));
        if let Some(comment) = &column.comment {
            self.comment_column(&quote(&column.name)?, Some(comment))?;
        }
        Ok(())
    }

    fn indexes(&mut self, staged: &StagedChanges<'_>) -> Result<(), AlterError> {
        for &name in &staged.index_drops {
            let index = relation(self.schema, name)?;
            self.out.statements.push(format!("DROP INDEX {index}"));
            self.warn(format!(
                "Dropping index {name:?} can slow down queries that use it."
            ));
            self.out.destructive = true;
        }
        for index in &staged.index_adds {
            let sql = keys::create_index(&self.relation, index)?;
            self.out.statements.push(sql);
        }
        for index in &staged.concurrent_indexes {
            let sql = keys::create_index(&self.relation, index)?;
            self.out
                .statements
                .push(sql.replacen("INDEX ", "INDEX CONCURRENTLY ", 1));
            self.out.run = openrows_driver_api::structure::AlterRun::Autocommit;
            self.warn("CREATE INDEX CONCURRENTLY runs outside a transaction and permits writes. Failure may leave an invalid index; inspect and drop it before retrying.".into());
        }
        if !staged.index_adds.is_empty() {
            self.warn("CREATE INDEX blocks writes to the table while it builds.".to_owned());
        }
        Ok(())
    }

    fn constraints(&mut self, staged: &StagedChanges<'_>) -> Result<(), AlterError> {
        for (keys, primary) in [(&staged.primary_keys, true), (&staged.unique_keys, false)] {
            for key in keys {
                let sql = openrows_driver_api::structure::key_sql(
                    key,
                    primary,
                    openrows_sql::Dialect::Postgres,
                )?;
                self.alter(&format!("ADD {sql}"));
                self.warn("Adding a key scans and locks the table; duplicate values or NULL primary-key values cause failure.".to_owned());
            }
        }
        for key in &staged.foreign_keys {
            let clause = keys::foreign_key(key, self.schema)?;
            self.alter(&format!("ADD {clause}"));
            self.warn(format!(
                "Adding a foreign key to {} locks both tables and fails if existing rows have no match.",
                key.target.table
            ));
        }
        for check in &staged.checks {
            let clause = keys::check(check)?;
            self.alter(&format!("ADD {clause}"));
            self.warn(
                "Adding a check scans the table and fails if existing rows violate it.".to_owned(),
            );
        }
        Ok(())
    }

    fn table(&mut self, staged: &StagedChanges<'_>) -> Result<(), AlterError> {
        if let Some(Comment(comment)) = staged.comment {
            self.out.statements.push(format!(
                "COMMENT ON TABLE {} IS {}",
                self.relation,
                text(comment)?
            ));
        }
        if let Some(name) = staged.rename {
            self.alter(&format!("RENAME TO {}", quote(name)?));
            self.warn("Code that uses the old table name stops working.".to_owned());
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "alter_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "structure_options_tests.rs"]
mod option_tests;
