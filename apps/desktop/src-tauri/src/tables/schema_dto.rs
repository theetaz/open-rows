// SPDX-License-Identifier: Apache-2.0
//! The structure-editing IPC contract: table detail for the Structure, Index,
//! Triggers and DDL views, and the typed change set `table_alter` turns into
//! DDL in Rust (the webview never sends SQL).
use super::dto::TableTarget;
use openrows_driver_api::schema::{DefaultValue, IndexMethod, NewColumn, NewIndex, SchemaChange};
use serde::{Deserialize, Serialize};
use specta::Type;

/// How a column's value is produced.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum GeneratedDto {
    No,
    IdentityAlways,
    IdentityByDefault,
    Stored,
}

/// A single-column foreign key: `→ table(column)`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ForeignKeyDto {
    pub(crate) schema: String,
    pub(crate) table: String,
    pub(crate) column: String,
}

/// One row of the Structure grid.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StructureColumnDto {
    pub(crate) name: String,
    pub(crate) data_type: String,
    pub(crate) nullable: bool,
    /// Default or generation expression as SQL text.
    pub(crate) default: Option<String>,
    pub(crate) generated: GeneratedDto,
    pub(crate) comment: Option<String>,
    /// Single-column check expressions, joined with ` AND `.
    pub(crate) check: Option<String>,
    pub(crate) foreign_key: Option<ForeignKeyDto>,
}

/// One row of the Index grid.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IndexDetailDto {
    pub(crate) name: String,
    pub(crate) method: String,
    pub(crate) unique: bool,
    pub(crate) primary: bool,
    /// Backs a constraint, so it cannot be dropped as an index.
    pub(crate) constraint: bool,
    pub(crate) columns: String,
    pub(crate) include: Vec<String>,
    pub(crate) predicate: Option<String>,
    pub(crate) comment: Option<String>,
    pub(crate) definition: String,
}

/// When a trigger fires.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum TriggerTimingDto {
    Before,
    After,
    InsteadOf,
}

/// One trigger in the Triggers sheet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TriggerDto {
    pub(crate) name: String,
    pub(crate) timing: TriggerTimingDto,
    /// `INSERT`, `UPDATE`, `DELETE`, `TRUNCATE`.
    pub(crate) events: Vec<String>,
    pub(crate) row_level: bool,
    pub(crate) function: String,
    pub(crate) enabled: bool,
    pub(crate) definition: String,
}

/// Kind of the browsed relation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RelationKindDto {
    Table,
    PartitionedTable,
    View,
    MaterializedView,
    ForeignTable,
}

/// Sizes in bytes, the planner estimate and the comment.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TableInfoDto {
    pub(crate) kind: RelationKindDto,
    pub(crate) row_estimate: Option<f64>,
    pub(crate) total_bytes: f64,
    pub(crate) data_bytes: f64,
    pub(crate) index_bytes: f64,
    pub(crate) comment: Option<String>,
}

/// Everything the Structure, Index, Triggers and DDL views and the inspector
/// table info show.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TableDetailDto {
    pub(crate) columns: Vec<StructureColumnDto>,
    pub(crate) primary_key: Vec<String>,
    pub(crate) indexes: Vec<IndexDetailDto>,
    pub(crate) triggers: Vec<TriggerDto>,
    pub(crate) info: TableInfoDto,
    /// Reconstructed definition (`-- Table Definition`, `CREATE TABLE …`).
    pub(crate) ddl: String,
    /// A table on a writable connection: structure changes can be staged.
    pub(crate) editable: bool,
}

/// A column default: none, a quoted text literal or a raw SQL expression.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum DefaultDto {
    None,
    Literal { value: String },
    Expression { sql: String },
}

/// Index access methods offered by `+ Index`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum IndexMethodDto {
    Btree,
    Hash,
    Gist,
    Gin,
    Brin,
    Spgist,
}

/// One staged structure change; columns are named by their current name.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum SchemaChangeDto {
    RenameTable {
        name: String,
    },
    TableComment {
        comment: Option<String>,
    },
    RenameColumn {
        column: String,
        name: String,
    },
    ColumnType {
        column: String,
        #[serde(rename = "dataType")]
        data_type: String,
    },
    ColumnNullable {
        column: String,
        nullable: bool,
    },
    ColumnDefault {
        column: String,
        default: DefaultDto,
    },
    ColumnComment {
        column: String,
        comment: Option<String>,
    },
    AddColumn {
        name: String,
        #[serde(rename = "dataType")]
        data_type: String,
        nullable: bool,
        default: DefaultDto,
        comment: Option<String>,
    },
    DropColumn {
        column: String,
    },
    CreateIndex {
        name: Option<String>,
        columns: Vec<String>,
        unique: bool,
        method: IndexMethodDto,
    },
    DropIndex {
        name: String,
    },
}

/// Preview (no token) or commit (the preview's token) of a change set.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AlterRequest {
    pub(crate) session_id: String,
    pub(crate) target: TableTarget,
    pub(crate) changes: Vec<SchemaChangeDto>,
    pub(crate) confirm_token: Option<String>,
    /// The typed table name, when the preview asked for it.
    pub(crate) confirm_name: Option<String>,
}

/// The generated DDL to review, or the committed result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum AlterReply {
    Prepared {
        token: String,
        statements: Vec<String>,
        warnings: Vec<String>,
        destructive: bool,
        /// Name to type before committing (destructive changes on a
        /// production or confirm-writes connection).
        #[serde(rename = "confirmName")]
        confirm_name: Option<String>,
    },
    Committed,
}

impl From<&DefaultDto> for DefaultValue {
    fn from(default: &DefaultDto) -> Self {
        match default {
            DefaultDto::None => Self::None,
            DefaultDto::Literal { value } => Self::Literal(value.clone()),
            DefaultDto::Expression { sql } => Self::Expression(sql.clone()),
        }
    }
}

impl From<IndexMethodDto> for IndexMethod {
    fn from(method: IndexMethodDto) -> Self {
        match method {
            IndexMethodDto::Btree => Self::Btree,
            IndexMethodDto::Hash => Self::Hash,
            IndexMethodDto::Gist => Self::Gist,
            IndexMethodDto::Gin => Self::Gin,
            IndexMethodDto::Brin => Self::Brin,
            IndexMethodDto::Spgist => Self::Spgist,
        }
    }
}

impl From<&SchemaChangeDto> for SchemaChange {
    fn from(change: &SchemaChangeDto) -> Self {
        match change.clone() {
            SchemaChangeDto::RenameTable { name } => Self::RenameTable(name),
            SchemaChangeDto::TableComment { comment } => Self::TableComment(comment),
            SchemaChangeDto::RenameColumn { column, name } => Self::RenameColumn { column, name },
            SchemaChangeDto::ColumnType { column, data_type } => {
                Self::ColumnType { column, data_type }
            }
            SchemaChangeDto::ColumnNullable { column, nullable } => {
                Self::ColumnNullable { column, nullable }
            }
            SchemaChangeDto::ColumnDefault { column, default } => Self::ColumnDefault {
                column,
                default: (&default).into(),
            },
            SchemaChangeDto::ColumnComment { column, comment } => {
                Self::ColumnComment { column, comment }
            }
            SchemaChangeDto::AddColumn {
                name,
                data_type,
                nullable,
                default,
                comment,
            } => Self::AddColumn(NewColumn {
                name,
                data_type,
                nullable,
                default: (&default).into(),
                comment,
            }),
            SchemaChangeDto::DropColumn { column } => Self::DropColumn(column),
            SchemaChangeDto::CreateIndex {
                name,
                columns,
                unique,
                method,
            } => Self::CreateIndex(NewIndex {
                name,
                columns,
                unique,
                method: method.into(),
            }),
            SchemaChangeDto::DropIndex { name } => Self::DropIndex(name),
        }
    }
}
