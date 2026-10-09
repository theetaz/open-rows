// SPDX-License-Identifier: Apache-2.0
//! A staged change set grouped per column and validated against the
//! catalog, shared by every engine's DDL generator.
use super::AlterTable;
use crate::schema::{
    DefaultValue, Identity, NewCheck, NewColumn, NewForeignKey, NewIndex, RelationKind,
    SchemaChange, TableDetail,
};
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
    /// The engine has no such feature (e.g. `SQLite` comments).
    #[error("{0}")]
    Unsupported(String),
    /// The server's own table definition could not be read.
    #[error("the definition of {0:?} could not be read; reload the structure")]
    UnreadableDefinition(String),
    /// The constraint is not in the catalog.
    #[error("the table has no constraint named {0:?}; reload the structure")]
    UnknownConstraint(String),
    /// A foreign key without columns, or with fewer referenced columns.
    #[error("foreign key {0:?} needs as many referenced columns as columns")]
    InvalidForeignKey(String),
    /// An empty or invalid check or predicate expression.
    #[error("the expression {0:?} is empty or invalid")]
    InvalidExpression(String),
    /// A new table without columns.
    #[error("a table needs at least one column")]
    NoColumns,
}

/// Which indexes `DROP INDEX` may remove.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IndexDrops {
    /// Not those backing a constraint (`PostgreSQL`, `SQLite`).
    NotConstraints,
    /// Any but the primary key (`MySQL`, where a unique key is its index).
    NotPrimary,
}

/// A comment to set (`Some`) or remove (`None`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Comment<'a>(pub Option<&'a str>);

/// A column's new character set and collation; `None` is the default.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Collation<'a> {
    /// Character set (`MySQL`).
    pub charset: Option<&'a str>,
    /// Collation name.
    pub collation: Option<&'a str>,
}

/// Every change staged for one existing column.
#[derive(Clone, Copy, Debug, Default)]
pub struct ColumnEdits<'a> {
    /// New name.
    pub rename: Option<&'a str>,
    /// New type as SQL type syntax.
    pub data_type: Option<&'a str>,
    /// New nullability.
    pub nullable: Option<bool>,
    /// New default.
    pub default: Option<&'a DefaultValue>,
    /// New comment.
    pub comment: Option<Comment<'a>>,
    /// New identity or auto-increment numbering.
    pub identity: Option<Identity>,
    /// New character set and collation.
    pub collation: Option<Collation<'a>>,
}

impl<'a> ColumnEdits<'a> {
    /// A rename to a different name.
    #[must_use]
    pub fn renamed(&self, current: &str) -> Option<&'a str> {
        self.rename.filter(|name| *name != current)
    }
}

/// A validated change set, grouped for generation.
#[derive(Clone, Debug, Default)]
pub struct StagedChanges<'a> {
    /// Edits per existing column (current names).
    pub columns: BTreeMap<&'a str, ColumnEdits<'a>>,
    /// Columns to drop (current names).
    pub drops: Vec<&'a str>,
    /// Columns to add.
    pub adds: Vec<&'a NewColumn>,
    /// Indexes to drop.
    pub index_drops: Vec<&'a str>,
    /// Indexes to create (final column names).
    pub index_adds: Vec<&'a NewIndex>,
    /// New table name.
    pub rename: Option<&'a str>,
    /// New table comment.
    pub comment: Option<Comment<'a>>,
    /// Foreign keys to add (final column names).
    pub foreign_keys: Vec<&'a NewForeignKey>,
    /// Check constraints to add.
    pub checks: Vec<&'a NewCheck>,
    /// Constraints to drop (names).
    pub constraint_drops: Vec<&'a str>,
}

impl<'a> StagedChanges<'a> {
    /// Group `changes` and check them against `target`'s catalog detail.
    ///
    /// # Errors
    /// See [`AlterError`].
    pub fn stage(
        target: &AlterTable<'_>,
        changes: &'a [SchemaChange],
        index_drops: IndexDrops,
    ) -> Result<Self, AlterError> {
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
        let mut staged = Self::default();
        for change in changes {
            staged.add(change)?;
        }
        staged.validate(target.detail, index_drops)?;
        Ok(staged)
    }

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
            SchemaChange::ColumnIdentity { column, identity } => {
                once(&mut self.edits(column).identity, *identity, column)
            }
            SchemaChange::ColumnCollation {
                column,
                charset,
                collation,
            } => once(
                &mut self.edits(column).collation,
                Collation {
                    charset: charset.as_deref(),
                    collation: collation.as_deref(),
                },
                column,
            ),
            SchemaChange::AddForeignKey(key) => {
                self.foreign_keys.push(key);
                Ok(())
            }
            SchemaChange::AddCheck(check) => {
                self.checks.push(check);
                Ok(())
            }
            SchemaChange::DropConstraint(name) => push_unique(&mut self.constraint_drops, name),
        }
    }

    fn edits(&mut self, column: &'a str) -> &mut ColumnEdits<'a> {
        self.columns.entry(column).or_default()
    }

    fn validate(&self, detail: &TableDetail, rule: IndexDrops) -> Result<(), AlterError> {
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
            keys::index(index, &finals)?;
        }
        for key in &self.foreign_keys {
            keys::foreign_key(key, &finals)?;
        }
        for check in &self.checks {
            keys::expression(&check.expression)?;
        }
        keys::constraint_drops(&self.constraint_drops, detail)?;
        self.validate_index_drops(detail, rule)
    }

    fn validate_index_drops(
        &self,
        detail: &TableDetail,
        rule: IndexDrops,
    ) -> Result<(), AlterError> {
        for &name in &self.index_drops {
            let index = detail.indexes.iter().find(|index| index.name == name);
            let refused = match (index, rule) {
                (None, _) => return Err(AlterError::UnknownIndex(name.to_owned())),
                (Some(index), IndexDrops::NotConstraints) => index.constraint,
                (Some(index), IndexDrops::NotPrimary) => index.primary,
            };
            if refused {
                return Err(AlterError::ConstraintIndex(name.to_owned()));
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
            let Some(name) = edits.renamed(column) else {
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

pub(crate) mod keys;

/// Store `value` in an empty `slot`; a second value for it is a conflict.
fn once<T>(slot: &mut Option<T>, value: T, what: &str) -> Result<(), AlterError> {
    if slot.is_some() {
        return Err(AlterError::Conflict(what.to_owned()));
    }
    *slot = Some(value);
    Ok(())
}

fn push_unique<'a>(list: &mut Vec<&'a str>, name: &'a str) -> Result<(), AlterError> {
    if list.contains(&name) {
        return Err(AlterError::Conflict(name.to_owned()));
    }
    list.push(name);
    Ok(())
}
