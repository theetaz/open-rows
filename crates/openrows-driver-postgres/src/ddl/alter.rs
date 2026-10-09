// SPDX-License-Identifier: Apache-2.0
//! A staged structure change set as ordered DDL for one transaction.
//!
//! Columns are named by their current catalog name. Statements run in this
//! order so each refers to names that exist at that point: drop columns;
//! per existing column (catalog order) type, nullability, default, comment
//! and finally the rename; add columns; drop then create indexes (on final
//! column names); the table comment; the table rename last.
use super::{data_type, default_sql, quote, relation, text};
use openrows_driver_api::schema::{IndexMethod, NewColumn, NewIndex, SchemaChange, TableDetail};
pub use openrows_driver_api::structure::{AlterError, AlterPlan, AlterTable};
use openrows_driver_api::structure::{ColumnEdits, Comment, IndexDrops, StagedChanges};

/// Validate `changes` against the catalog and generate ordered DDL.
///
/// # Errors
/// See [`AlterError`].
pub fn alter_sql(
    target: &AlterTable<'_>,
    changes: &[SchemaChange],
) -> Result<AlterPlan, AlterError> {
    let staged = StagedChanges::stage(target, changes, IndexDrops::NotConstraints)?;
    let mut plan = Plan {
        relation: relation(target.schema, target.table)?,
        schema: target.schema,
        out: AlterPlan::default(),
    };
    plan.columns(target.detail, &staged)?;
    plan.indexes(&staged)?;
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

    fn columns(
        &mut self,
        detail: &TableDetail,
        staged: &StagedChanges<'_>,
    ) -> Result<(), AlterError> {
        for &name in &staged.drops {
            self.alter(&format!("DROP COLUMN {}", quote(name)?));
            self.warn(format!(
                "Dropping column {name:?} permanently deletes its data."
            ));
            self.out.destructive = true;
        }
        for column in &detail.columns {
            if let Some(edits) = staged.columns.get(column.name.as_str()) {
                self.column(&column.name, edits)?;
            }
        }
        for column in &staged.adds {
            self.add_column(column)?;
        }
        Ok(())
    }

    fn column(&mut self, name: &str, edits: &ColumnEdits<'_>) -> Result<(), AlterError> {
        let quoted = quote(name)?;
        if let Some(text) = edits.data_type {
            self.alter(&format!("ALTER COLUMN {quoted} TYPE {}", data_type(text)?));
            self.warn(format!(
                "Changing the type of {name:?} rewrites the table under an exclusive lock and fails if values cannot be cast."
            ));
            self.out.destructive = true;
        }
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
        if let Some(Comment(comment)) = edits.comment {
            self.comment_column(&quoted, comment)?;
        }
        if let Some(rename) = edits.rename.filter(|rename| *rename != name) {
            self.alter(&format!("RENAME COLUMN {quoted} TO {}", quote(rename)?));
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
        let quoted = quote(&column.name)?;
        let mut action = format!("ADD COLUMN {quoted} {}", data_type(&column.data_type)?);
        if !column.nullable {
            action.push_str(" NOT NULL");
        }
        match default_sql(&column.name, &column.default)? {
            Some(sql) => action = format!("{action} DEFAULT {sql}"),
            None if !column.nullable => self.warn(format!(
                "Adding NOT NULL column {:?} without a default fails if the table has rows.",
                column.name
            )),
            None => {}
        }
        self.alter(&action);
        if let Some(comment) = &column.comment {
            self.comment_column(&quoted, Some(comment))?;
        }
        Ok(())
    }

    fn indexes(&mut self, staged: &StagedChanges<'_>) -> Result<(), AlterError> {
        for &name in &staged.index_drops {
            let index = relation(self.schema, name)?;
            self.out.statements.push(format!("DROP INDEX {index}"));
            self.out.destructive = true;
        }
        for index in &staged.index_adds {
            let sql = self.create_index(index)?;
            self.out.statements.push(sql);
        }
        if !staged.index_adds.is_empty() {
            self.warn("CREATE INDEX blocks writes to the table while it builds.".to_owned());
        }
        Ok(())
    }

    fn create_index(&self, index: &NewIndex) -> Result<String, AlterError> {
        let unique = if index.unique { "UNIQUE " } else { "" };
        let name = match index.name.as_deref().filter(|name| !name.is_empty()) {
            Some(name) => format!("{} ", quote(name)?),
            None => String::new(),
        };
        let columns = index
            .columns
            .iter()
            .map(|column| quote(column))
            .collect::<Result<Vec<_>, _>>()?
            .join(", ");
        Ok(format!(
            "CREATE {unique}INDEX {name}ON {} USING {} ({columns})",
            self.relation,
            method(index.method)
        ))
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

fn method(method: IndexMethod) -> &'static str {
    match method {
        IndexMethod::Btree => "btree",
        IndexMethod::Hash => "hash",
        IndexMethod::Gist => "gist",
        IndexMethod::Gin => "gin",
        IndexMethod::Brin => "brin",
        IndexMethod::Spgist => "spgist",
    }
}

#[cfg(test)]
#[path = "alter_tests.rs"]
mod tests;
