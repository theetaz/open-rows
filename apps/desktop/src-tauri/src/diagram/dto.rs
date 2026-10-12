// SPDX-License-Identifier: Apache-2.0
//! The ER diagram IPC contract.
use openrows_driver_api::diagram::{DiagramColumn, DiagramForeignKey, DiagramTable, SchemaDiagram};
use serde::{Deserialize, Serialize};
use specta::Type;

/// The schema a diagram is read for.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiagramTarget {
    pub(crate) profile_id: String,
    pub(crate) database: String,
    pub(crate) schema: String,
    /// Exact table names; absent means the whole schema.
    #[serde(default)]
    pub(crate) tables: Option<Vec<String>>,
}

/// One column of a diagram table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiagramColumnDto {
    name: String,
    data_type: String,
    nullable: bool,
    primary_key: bool,
}

/// One table with its columns in ordinal order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiagramTableDto {
    name: String,
    columns: Vec<DiagramColumnDto>,
}

/// A foreign key between two tables of the diagram.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiagramForeignKeyDto {
    name: String,
    table: String,
    columns: Vec<String>,
    target_table: String,
    target_columns: Vec<String>,
}

/// Tables and foreign keys of a schema; `truncated` when it has more than
/// `limit` tables and only the first `limit` (by name) are shown.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SchemaDiagramDto {
    tables: Vec<DiagramTableDto>,
    foreign_keys: Vec<DiagramForeignKeyDto>,
    truncated: bool,
    limit: u32,
}

impl SchemaDiagramDto {
    pub(crate) fn new(diagram: SchemaDiagram, limit: usize) -> Self {
        Self {
            tables: diagram.tables.into_iter().map(table).collect(),
            foreign_keys: diagram.foreign_keys.into_iter().map(key).collect(),
            truncated: diagram.truncated,
            limit: u32::try_from(limit).unwrap_or(u32::MAX),
        }
    }
}

fn table(table: DiagramTable) -> DiagramTableDto {
    DiagramTableDto {
        name: table.name,
        columns: table.columns.into_iter().map(column).collect(),
    }
}

fn column(column: DiagramColumn) -> DiagramColumnDto {
    DiagramColumnDto {
        name: column.name,
        data_type: column.data_type,
        nullable: column.nullable,
        primary_key: column.primary_key,
    }
}

fn key(key: DiagramForeignKey) -> DiagramForeignKeyDto {
    DiagramForeignKeyDto {
        name: key.name,
        table: key.table,
        columns: key.columns,
        target_table: key.target.table,
        target_columns: key.target.columns,
    }
}

/// An exported diagram: SVG text or PNG bytes.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Type)]
#[serde(rename_all = "camelCase", tag = "format")]
pub(crate) enum DiagramFileDto {
    Svg { text: String },
    Png { bytes: Vec<u8> },
}

impl DiagramFileDto {
    /// File extension and the save panel's filter name.
    pub(crate) fn kind(&self) -> (&'static str, &'static str) {
        match self {
            Self::Svg { .. } => ("svg", "SVG image"),
            Self::Png { .. } => ("png", "PNG image"),
        }
    }

    pub(crate) fn contents(&self) -> &[u8] {
        match self {
            Self::Svg { text } => text.as_bytes(),
            Self::Png { bytes } => bytes,
        }
    }
}
