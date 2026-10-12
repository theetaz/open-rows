// SPDX-License-Identifier: Apache-2.0
//! The structure-editing IPC contract: table detail for the Structure, Index,
//! Triggers and DDL views, and the typed change set `table_alter` turns into
//! DDL in Rust (the webview never sends SQL).
use super::constraint_dto::{ConstraintDto, IdentityDto, NewCheckDto, NewForeignKeyDto};
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
    AutoIncrement,
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

/// One partition of a partitioned table.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PartitionDto {
    pub(crate) schema: String,
    pub(crate) name: String,
    /// `FOR VALUES …` as the server prints it.
    pub(crate) bound: String,
    /// Itself partitioned.
    pub(crate) partitioned: bool,
    pub(crate) row_estimate: Option<f64>,
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
    /// Partitions of a partitioned table, by name.
    pub(crate) partitions: Vec<PartitionDto>,
    pub(crate) info: TableInfoDto,
    /// Every constraint: primary key, unique, check, foreign key, exclusion.
    pub(crate) constraints: Vec<ConstraintDto>,
    /// The query of a view or materialized view.
    pub(crate) view_query: Option<String>,
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

/// A key constraint added to an existing table.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NewKeyDto {
    name: Option<String>,
    columns: Vec<String>,
}

impl From<NewKeyDto> for openrows_driver_api::schema::NewKey {
    fn from(key: NewKeyDto) -> Self {
        Self {
            name: key.name,
            columns: key.columns,
        }
    }
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
        #[serde(default)]
        identity: IdentityDto,
        #[serde(default)]
        collation: Option<String>,
        #[serde(default)]
        charset: Option<String>,
    },
    DropColumn {
        column: String,
    },
    CreateIndex {
        name: Option<String>,
        columns: Vec<String>,
        unique: bool,
        method: IndexMethodDto,
        /// Key columns sorted descending.
        #[serde(default)]
        descending: Vec<String>,
        /// Partial-index predicate.
        #[serde(default)]
        predicate: Option<String>,
    },
    DropIndex {
        name: String,
    },
    ColumnIdentity {
        column: String,
        identity: IdentityDto,
    },
    ColumnCollation {
        column: String,
        charset: Option<String>,
        collation: Option<String>,
    },
    AddForeignKey {
        key: NewForeignKeyDto,
    },
    AddCheck {
        check: NewCheckDto,
    },
    AddPrimaryKey {
        key: NewKeyDto,
    },
    AddUnique {
        key: NewKeyDto,
    },
    ColumnTypeUsing {
        column: String,
        #[serde(rename = "dataType")]
        data_type: String,
        expression: String,
    },
    CreateIndexConcurrently {
        index: super::object_dto::NewIndexDto,
    },
    ReorderColumn {
        column: String,
        after: Option<String>,
    },
    DropConstraint {
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
        transactional: bool,
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

impl TryFrom<&SchemaChangeDto> for SchemaChange {
    type Error = openrows_driver_api::structure::AlterError;
    fn try_from(change: &SchemaChangeDto) -> Result<Self, Self::Error> {
        if let Some(column) = column_change(change.clone()) {
            return Ok(column);
        }
        Ok(match change.clone() {
            SchemaChangeDto::RenameTable { name } => Self::RenameTable(name),
            SchemaChangeDto::TableComment { comment } => Self::TableComment(comment),
            SchemaChangeDto::AddColumn {
                name,
                data_type,
                nullable,
                default,
                comment,
                identity,
                collation,
                charset,
            } => Self::AddColumn(NewColumn {
                name,
                data_type,
                nullable,
                default: (&default).into(),
                comment,
                identity: identity.into(),
                collation,
                charset,
            }),
            SchemaChangeDto::DropColumn { column } => Self::DropColumn(column),
            SchemaChangeDto::CreateIndex {
                name,
                columns,
                unique,
                method,
                descending,
                predicate,
            } => Self::CreateIndex(NewIndex {
                name,
                columns,
                unique,
                method: method.into(),
                descending,
                predicate,
            }),
            SchemaChangeDto::DropIndex { name } => Self::DropIndex(name),
            SchemaChangeDto::AddForeignKey { key } => Self::AddForeignKey(key.into()),
            SchemaChangeDto::AddCheck { check } => Self::AddCheck(check.into()),
            SchemaChangeDto::AddPrimaryKey { key } => Self::AddPrimaryKey(key.into()),
            SchemaChangeDto::AddUnique { key } => Self::AddUnique(key.into()),
            SchemaChangeDto::CreateIndexConcurrently { index } => {
                Self::CreateIndexConcurrently(index.into())
            }
            SchemaChangeDto::DropConstraint { name } => Self::DropConstraint(name),
            _ => return Err(Self::Error::Unsupported("Unknown column change".into())),
        })
    }
}

fn column_change(change: SchemaChangeDto) -> Option<SchemaChange> {
    Some(match change {
        SchemaChangeDto::RenameColumn { column, name } => {
            SchemaChange::RenameColumn { column, name }
        }
        SchemaChangeDto::ColumnType { column, data_type } => {
            SchemaChange::ColumnType { column, data_type }
        }
        SchemaChangeDto::ColumnNullable { column, nullable } => {
            SchemaChange::ColumnNullable { column, nullable }
        }
        SchemaChangeDto::ColumnDefault { column, default } => SchemaChange::ColumnDefault {
            column,
            default: (&default).into(),
        },
        SchemaChangeDto::ColumnComment { column, comment } => {
            SchemaChange::ColumnComment { column, comment }
        }
        SchemaChangeDto::ColumnIdentity { column, identity } => SchemaChange::ColumnIdentity {
            column,
            identity: identity.into(),
        },
        SchemaChangeDto::ColumnCollation {
            column,
            charset,
            collation,
        } => SchemaChange::ColumnCollation {
            column,
            charset,
            collation,
        },
        SchemaChangeDto::ColumnTypeUsing {
            column,
            data_type,
            expression,
        } => SchemaChange::ColumnTypeUsing {
            column,
            data_type,
            expression,
        },
        SchemaChangeDto::ReorderColumn { column, after } => {
            SchemaChange::ReorderColumn { column, after }
        }
        _ => return None,
    })
}
