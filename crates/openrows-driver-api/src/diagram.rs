// SPDX-License-Identifier: Apache-2.0
//! The read-only ER diagram of one schema (FR-SCHEMA-06): its tables with
//! their columns, and the foreign keys between them.
//!
//! Engines with a server catalog read the whole schema in two bulk queries
//! and assemble the rows with [`assemble`]; [`read_per_table`] is the
//! fallback for engines whose per-table reads are local and cheap.
use std::collections::{HashMap, HashSet};

use crate::catalog::ObjectKind;
use crate::schema::{ConstraintKind, ForeignTarget, TableDetail};
use crate::{Catalog, DriverError};

/// One column as the diagram shows it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DiagramColumn {
    /// Column name.
    pub name: String,
    /// Formatted type, e.g. `character varying(40)`.
    pub data_type: String,
    /// `false` for `NOT NULL` columns.
    pub nullable: bool,
    /// Part of the primary key.
    pub primary_key: bool,
}

/// One table with its columns in ordinal order.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DiagramTable {
    /// Table name.
    pub name: String,
    /// Columns in ordinal order.
    pub columns: Vec<DiagramColumn>,
}

/// A foreign key between two tables of the diagram.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DiagramForeignKey {
    /// Constraint name.
    pub name: String,
    /// Referencing table.
    pub table: String,
    /// Referencing columns, in key order.
    pub columns: Vec<String>,
    /// Referenced table and columns.
    pub target: ForeignTarget,
}

/// Tables and foreign keys of one schema.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SchemaDiagram {
    /// Tables ordered by name, at most the requested number.
    pub tables: Vec<DiagramTable>,
    /// Foreign keys whose both ends are among `tables`.
    pub foreign_keys: Vec<DiagramForeignKey>,
    /// The schema has more tables than were read.
    pub truncated: bool,
}

/// One row of a bulk column read; `column` is `None` for a table without
/// columns (a left join that found none).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ColumnRow {
    /// Owning table.
    pub table: String,
    /// The column, when the table has any.
    pub column: Option<DiagramColumn>,
}

/// One column pair of a foreign key from a bulk read, in key order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForeignKeyRow {
    /// Referencing table.
    pub table: String,
    /// Constraint name.
    pub name: String,
    /// Referencing column.
    pub column: String,
    /// Referenced schema.
    pub target_schema: String,
    /// Referenced table.
    pub target_table: String,
    /// Referenced column; empty when the engine leaves it implicit.
    pub target_column: String,
}

/// Group bulk rows (tables in name order, columns in ordinal order, key
/// columns in key order) into a diagram of at most `max_tables` tables of
/// `schema`, keeping only foreign keys between those tables.
#[must_use]
pub fn assemble(
    schema: &str,
    columns: Vec<ColumnRow>,
    keys: Vec<ForeignKeyRow>,
    max_tables: usize,
) -> SchemaDiagram {
    let mut tables: Vec<DiagramTable> = Vec::new();
    for row in columns {
        if tables.last().is_none_or(|table| table.name != row.table) {
            tables.push(DiagramTable {
                name: row.table,
                columns: Vec::new(),
            });
        }
        if let (Some(table), Some(column)) = (tables.last_mut(), row.column) {
            table.columns.push(column);
        }
    }
    let truncated = tables.len() > max_tables;
    tables.truncate(max_tables);
    let foreign_keys = group_keys(keys);
    visible(
        schema,
        SchemaDiagram {
            tables,
            foreign_keys,
            truncated,
        },
    )
}

fn group_keys(rows: Vec<ForeignKeyRow>) -> Vec<DiagramForeignKey> {
    let mut keys: Vec<DiagramForeignKey> = Vec::new();
    for row in rows {
        let same = keys
            .last()
            .is_some_and(|key| key.table == row.table && key.name == row.name);
        if !same {
            keys.push(DiagramForeignKey {
                name: row.name,
                table: row.table,
                columns: Vec::new(),
                target: ForeignTarget {
                    schema: row.target_schema,
                    table: row.target_table,
                    columns: Vec::new(),
                },
            });
        }
        if let Some(key) = keys.last_mut() {
            key.columns.push(row.column);
            key.target.columns.push(row.target_column);
        }
    }
    keys
}

/// Keep foreign keys whose both ends are diagram tables of `schema`,
/// resolving a target written in another letter case (`SQLite` and
/// case-insensitive `MySQL` names) to the listed table.
fn visible(schema: &str, mut diagram: SchemaDiagram) -> SchemaDiagram {
    let names: HashSet<&str> = diagram.tables.iter().map(|t| t.name.as_str()).collect();
    let folded: HashMap<String, &str> = diagram
        .tables
        .iter()
        .map(|table| (table.name.to_lowercase(), table.name.as_str()))
        .collect();
    let resolve = |name: &str| -> Option<String> {
        if names.contains(name) {
            return Some(name.to_owned());
        }
        folded
            .get(&name.to_lowercase())
            .map(|&name| name.to_owned())
    };
    let keys = std::mem::take(&mut diagram.foreign_keys);
    diagram.foreign_keys = keys
        .into_iter()
        .filter(|key| names.contains(key.table.as_str()))
        .filter(|key| key.target.schema.eq_ignore_ascii_case(schema))
        .filter_map(|mut key| {
            key.target.table = resolve(&key.target.table)?;
            Some(key)
        })
        .collect();
    diagram
}

/// The diagram from per-table reads: list the tables, then read each one's
/// detail. Fine for local engines; server engines override
/// [`Catalog::schema_diagram`] with bulk queries.
///
/// # Errors
/// Server or transport failures of any read.
pub async fn read_per_table<C: Catalog + ?Sized>(
    catalog: &mut C,
    schema: &str,
    max_tables: usize,
) -> Result<SchemaDiagram, DriverError> {
    let objects = catalog.list_objects(schema, &[ObjectKind::Table]).await?;
    let mut columns = Vec::new();
    let mut keys = Vec::new();
    for object in objects.iter().take(max_tables.saturating_add(1)) {
        let detail = catalog.table_detail(schema, &object.name).await?;
        detail_rows(&object.name, &detail, (&mut columns, &mut keys));
    }
    Ok(assemble(schema, columns, keys, max_tables))
}

/// Flatten one table's detail into bulk rows.
fn detail_rows(
    table: &str,
    detail: &TableDetail,
    (columns, keys): (&mut Vec<ColumnRow>, &mut Vec<ForeignKeyRow>),
) {
    let primary: HashSet<&str> = detail
        .constraints
        .iter()
        .filter(|constraint| constraint.kind == ConstraintKind::Primary)
        .flat_map(|constraint| constraint.columns.iter().map(String::as_str))
        .collect();
    columns.push(ColumnRow {
        table: table.to_owned(),
        column: None,
    });
    columns.extend(detail.columns.iter().map(|column| ColumnRow {
        table: table.to_owned(),
        column: Some(DiagramColumn {
            name: column.name.clone(),
            data_type: column.data_type.clone(),
            nullable: column.nullable,
            primary_key: primary.contains(column.name.as_str()),
        }),
    }));
    for constraint in &detail.constraints {
        let Some(target) = constraint.references.as_ref() else {
            continue;
        };
        for (index, column) in constraint.columns.iter().enumerate() {
            keys.push(ForeignKeyRow {
                table: table.to_owned(),
                name: constraint.name.clone(),
                column: column.clone(),
                target_schema: target.schema.clone(),
                target_table: target.table.clone(),
                target_column: target.columns.get(index).cloned().unwrap_or_default(),
            });
        }
    }
}

#[cfg(test)]
#[path = "diagram_tests.rs"]
mod tests;
