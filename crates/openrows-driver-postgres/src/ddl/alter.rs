// SPDX-License-Identifier: Apache-2.0
//! A staged structure change set as ordered DDL for one transaction.
//!
//! Columns are named by their current catalog name. Statements run in this
//! order so each refers to names that exist at that point: drop columns;
//! per existing column (catalog order) type, nullability, default, comment
//! and finally the rename; add columns; drop then create indexes (on final
//! column names); the table comment; the table rename last.
use super::{data_type, default_sql, quote, relation, text};
use openrows_driver_api::schema::{
    DefaultValue, IndexMethod, NewColumn, NewIndex, RelationKind, SchemaChange, TableDetail,
};
pub use openrows_driver_api::structure::{AlterPlan, AlterTable};
use std::collections::{BTreeMap, BTreeSet};

/// Why a change set cannot be turned into DDL.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum AlterError {
    /// Nothing is staged.
    #[error("there are no structure changes to apply")]
    Empty,
    /// Views, materialized views and foreign tables are not altered here.
    #[error("only tables can be altered here")]
    NotATable,
    /// The column is not in the catalog (it may have changed meanwhile).
    #[error("the table has no column named {0:?}; reload the structure")]
    UnknownColumn(String),
    /// The index is not in the catalog.
    #[error("the table has no index named {0:?}; reload the structure")]
    UnknownIndex(String),
    /// Two columns would end up with the same name.
    #[error("a column named {0:?} already exists")]
    ColumnExists(String),
    /// The index backs a primary key, unique or exclusion constraint.
    #[error("index {0:?} backs a constraint; drop the constraint instead")]
    ConstraintIndex(String),
    /// Two changes set the same property, or change a dropped column.
    #[error("conflicting changes for {0:?}")]
    Conflict(String),
    /// Empty names or names with NUL characters.
    #[error("names cannot be empty or contain NUL characters")]
    InvalidName,
    /// The type text is not plain type syntax.
    #[error("{0:?} is not a valid type name")]
    InvalidType(String),
    /// An empty or invalid default expression.
    #[error("the default of {0:?} is empty or invalid")]
    InvalidDefault(String),
    /// `CREATE INDEX` without columns.
    #[error("an index needs at least one column")]
    NoIndexColumns,
}

/// A comment to set (`Some`) or remove (`None`).
#[derive(Clone, Copy)]
struct Comment<'a>(Option<&'a str>);

#[derive(Default)]
struct ColumnEdits<'a> {
    rename: Option<&'a str>,
    data_type: Option<&'a str>,
    nullable: Option<bool>,
    default: Option<&'a DefaultValue>,
    comment: Option<Comment<'a>>,
}

#[derive(Default)]
struct Staged<'a> {
    columns: BTreeMap<&'a str, ColumnEdits<'a>>,
    drops: Vec<&'a str>,
    adds: Vec<&'a NewColumn>,
    index_drops: Vec<&'a str>,
    index_adds: Vec<&'a NewIndex>,
    rename: Option<&'a str>,
    comment: Option<Comment<'a>>,
}

/// Validate `changes` against the catalog and generate ordered DDL.
///
/// # Errors
/// See [`AlterError`].
pub fn alter_sql(
    target: &AlterTable<'_>,
    changes: &[SchemaChange],
) -> Result<AlterPlan, AlterError> {
    let kind = target.detail.info.as_ref().map(|info| info.kind);
    if !matches!(
        kind,
        Some(RelationKind::Table | RelationKind::PartitionedTable)
    ) {
        return Err(AlterError::NotATable);
    }
    if changes.is_empty() {
        return Err(AlterError::Empty);
    }
    let mut staged = Staged::default();
    for change in changes {
        staged.add(change)?;
    }
    staged.validate(target.detail)?;
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

/// Store `value` in an empty `slot`; a second value for it is a conflict.
fn once<T>(slot: &mut Option<T>, value: T, what: &str) -> Result<(), AlterError> {
    if slot.is_some() {
        return Err(AlterError::Conflict(what.to_owned()));
    }
    *slot = Some(value);
    Ok(())
}

impl<'a> Staged<'a> {
    fn add(&mut self, change: &'a SchemaChange) -> Result<(), AlterError> {
        match change {
            SchemaChange::RenameTable(name) => once(&mut self.rename, name.as_str(), "table name"),
            SchemaChange::TableComment(comment) => once(
                &mut self.comment,
                Comment(comment.as_deref()),
                "table comment",
            ),
            SchemaChange::RenameColumn { column, name } => {
                once(&mut self.edits(column).rename, name.as_str(), column)
            }
            SchemaChange::ColumnType { column, data_type } => once(
                &mut self.edits(column).data_type,
                data_type.as_str(),
                column,
            ),
            SchemaChange::ColumnNullable { column, nullable } => {
                once(&mut self.edits(column).nullable, *nullable, column)
            }
            SchemaChange::ColumnDefault { column, default } => {
                once(&mut self.edits(column).default, default, column)
            }
            SchemaChange::ColumnComment { column, comment } => once(
                &mut self.edits(column).comment,
                Comment(comment.as_deref()),
                column,
            ),
            SchemaChange::AddColumn(column) => {
                self.adds.push(column);
                Ok(())
            }
            SchemaChange::DropColumn(column) => push_unique(&mut self.drops, column),
            SchemaChange::CreateIndex(index) => {
                self.index_adds.push(index);
                Ok(())
            }
            SchemaChange::DropIndex(name) => push_unique(&mut self.index_drops, name),
        }
    }

    fn edits(&mut self, column: &'a str) -> &mut ColumnEdits<'a> {
        self.columns.entry(column).or_default()
    }

    fn validate(&self, detail: &TableDetail) -> Result<(), AlterError> {
        let current: BTreeSet<&str> = detail.columns.iter().map(|c| c.name.as_str()).collect();
        for &name in self.columns.keys().chain(&self.drops) {
            if !current.contains(name) {
                return Err(AlterError::UnknownColumn(name.to_owned()));
            }
        }
        if let Some(name) = self
            .drops
            .iter()
            .find(|name| self.columns.contains_key(*name))
        {
            return Err(AlterError::Conflict((*name).to_owned()));
        }
        let finals = self.final_names(&current)?;
        for index in &self.index_adds {
            if index.columns.is_empty() {
                return Err(AlterError::NoIndexColumns);
            }
            if let Some(name) = index
                .columns
                .iter()
                .find(|name| !finals.contains(name.as_str()))
            {
                return Err(AlterError::UnknownColumn(name.clone()));
            }
        }
        for &name in &self.index_drops {
            match detail.indexes.iter().find(|index| index.name == name) {
                None => return Err(AlterError::UnknownIndex(name.to_owned())),
                Some(index) if index.constraint => {
                    return Err(AlterError::ConstraintIndex(name.to_owned()));
                }
                Some(_) => {}
            }
        }
        Ok(())
    }

    /// Column names after the change set; a rename may not take the name of
    /// another column that still exists while the renames run.
    fn final_names(&self, current: &BTreeSet<&'a str>) -> Result<BTreeSet<&'a str>, AlterError> {
        let mut finals: BTreeSet<&str> = current
            .iter()
            .filter(|name| !self.drops.contains(name))
            .copied()
            .collect();
        for (&column, edits) in &self.columns {
            let Some(name) = edits.rename.filter(|name| *name != column) else {
                continue;
            };
            if finals.contains(name) {
                return Err(AlterError::ColumnExists(name.to_owned()));
            }
            finals.remove(column);
            finals.insert(name);
        }
        for column in &self.adds {
            if !finals.insert(column.name.as_str()) {
                return Err(AlterError::ColumnExists(column.name.clone()));
            }
        }
        Ok(finals)
    }
}

fn push_unique<'a>(list: &mut Vec<&'a str>, name: &'a str) -> Result<(), AlterError> {
    if list.contains(&name) {
        return Err(AlterError::Conflict(name.to_owned()));
    }
    list.push(name);
    Ok(())
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

    fn columns(&mut self, detail: &TableDetail, staged: &Staged<'_>) -> Result<(), AlterError> {
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

    fn indexes(&mut self, staged: &Staged<'_>) -> Result<(), AlterError> {
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

    fn table(&mut self, staged: &Staged<'_>) -> Result<(), AlterError> {
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
