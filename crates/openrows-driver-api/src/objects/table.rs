// SPDX-License-Identifier: Apache-2.0
//! Checks of a new table's definition shared by every engine's generator.
use super::NewTable;
use crate::structure::AlterError;
use crate::structure::staged::keys;
use std::collections::BTreeSet;

impl NewTable {
    /// Columns are named uniquely; keys, indexes, foreign keys and checks
    /// name existing columns and valid expressions.
    ///
    /// # Errors
    /// See [`AlterError`].
    pub fn validate(&self) -> Result<(), AlterError> {
        if self.columns.is_empty() {
            return Err(AlterError::NoColumns);
        }
        let mut names = BTreeSet::new();
        for column in &self.columns {
            if column.name.trim().is_empty() {
                return Err(AlterError::InvalidName);
            }
            if !names.insert(column.name.as_str()) {
                return Err(AlterError::ColumnExists(column.name.clone()));
            }
        }
        let mut keyed = BTreeSet::new();
        for column in &self.primary_key {
            if !names.contains(column.as_str()) {
                return Err(AlterError::UnknownColumn(column.clone()));
            }
            if !keyed.insert(column.as_str()) {
                return Err(AlterError::Conflict(column.clone()));
            }
        }
        for index in &self.indexes {
            keys::index(index, &names)?;
        }
        for key in &self.foreign_keys {
            keys::foreign_key(key, &names)?;
        }
        for check in &self.checks {
            keys::expression(&check.expression)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{ForeignTarget, NewColumn, NewForeignKey, NewIndex};

    fn table() -> NewTable {
        let column = |name: &str| NewColumn {
            name: name.into(),
            data_type: "integer".into(),
            nullable: true,
            ..NewColumn::default()
        };
        NewTable {
            columns: vec![column("id"), column("owner_id")],
            primary_key: vec!["id".into()],
            ..NewTable::default()
        }
    }

    #[test]
    fn valid_tables_pass() -> Result<(), AlterError> {
        let mut definition = table();
        definition.indexes.push(NewIndex {
            columns: vec!["owner_id".into()],
            ..NewIndex::default()
        });
        definition.foreign_keys.push(NewForeignKey {
            columns: vec!["owner_id".into()],
            target: ForeignTarget {
                schema: "public".into(),
                table: "owners".into(),
                columns: vec!["id".into()],
            },
            ..NewForeignKey::default()
        });
        definition.validate()
    }

    #[test]
    fn broken_definitions_are_refused() {
        let mut empty = table();
        empty.columns.clear();
        assert_eq!(empty.validate(), Err(AlterError::NoColumns));
        let mut twice = table();
        twice.columns.push(twice.columns[0].clone());
        assert_eq!(twice.validate(), Err(AlterError::ColumnExists("id".into())));
        let mut unknown_key = table();
        unknown_key.primary_key = vec!["nope".into()];
        assert_eq!(
            unknown_key.validate(),
            Err(AlterError::UnknownColumn("nope".into()))
        );
        let mut bad_key = table();
        bad_key.foreign_keys.push(NewForeignKey {
            columns: vec!["owner_id".into()],
            ..NewForeignKey::default()
        });
        assert!(matches!(
            bad_key.validate(),
            Err(AlterError::InvalidForeignKey(_))
        ));
    }
}
