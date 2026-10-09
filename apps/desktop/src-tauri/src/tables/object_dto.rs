// SPDX-License-Identifier: Apache-2.0
//! The object lifecycle IPC contract (FR-SCHEMA-10, 11, 12): typed
//! create, rename, duplicate, truncate, save and drop requests. Names and
//! options are data; view queries and trigger and routine definitions are
//! the only SQL, checked by the driver before anything runs.
use super::constraint_dto::{IdentityDto, NewCheckDto, NewForeignKeyDto};
use super::schema_dto::{DefaultDto, IndexMethodDto};
use openrows_driver_api::objects::{NewTable, ObjectChange, ObjectName, RoutineKind};
use openrows_driver_api::schema::{NewColumn, NewIndex};
use serde::{Deserialize, Serialize};
use specta::Type;

/// A column of a new table.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NewColumnDto {
    pub(crate) name: String,
    pub(crate) data_type: String,
    pub(crate) nullable: bool,
    pub(crate) default: DefaultDto,
    pub(crate) comment: Option<String>,
    pub(crate) identity: IdentityDto,
    pub(crate) collation: Option<String>,
    pub(crate) charset: Option<String>,
}

/// An index of a new table.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NewIndexDto {
    pub(crate) name: Option<String>,
    pub(crate) columns: Vec<String>,
    pub(crate) unique: bool,
    pub(crate) method: IndexMethodDto,
    pub(crate) descending: Vec<String>,
    pub(crate) predicate: Option<String>,
}

/// The definition of a new table.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NewTableDto {
    pub(crate) columns: Vec<NewColumnDto>,
    pub(crate) primary_key: Vec<String>,
    pub(crate) indexes: Vec<NewIndexDto>,
    pub(crate) foreign_keys: Vec<NewForeignKeyDto>,
    pub(crate) checks: Vec<NewCheckDto>,
    pub(crate) comment: Option<String>,
}

/// Function or procedure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RoutineKindDto {
    Function,
    Procedure,
}

/// A schema-qualified object (the database itself is the schema in `MySQL`).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ObjectNameDto {
    pub(crate) schema: String,
    pub(crate) name: String,
}

/// One lifecycle change, tagged by `kind`: a table or view change, or a
/// trigger, routine, database or schema change.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(untagged)]
pub(crate) enum ObjectChangeDto {
    Relation(RelationChangeDto),
    Definition(DefinitionChangeDto),
    Container(ContainerChangeDto),
}

/// A table or view change; `target` is the table or view.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum RelationChangeDto {
    CreateTable {
        target: ObjectNameDto,
        definition: NewTableDto,
    },
    RenameTable {
        target: ObjectNameDto,
        name: String,
    },
    DuplicateTable {
        target: ObjectNameDto,
        name: String,
        #[serde(rename = "withData")]
        with_data: bool,
    },
    TruncateTable {
        target: ObjectNameDto,
        cascade: bool,
    },
    DropTable {
        target: ObjectNameDto,
        cascade: bool,
    },
    SaveView {
        target: ObjectNameDto,
        query: String,
        materialized: bool,
        replace: bool,
    },
    DropView {
        target: ObjectNameDto,
        materialized: bool,
        cascade: bool,
    },
}

/// A trigger (`target` is its table) or routine change.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum DefinitionChangeDto {
    SaveTrigger {
        target: ObjectNameDto,
        replaces: Option<String>,
        definition: String,
    },
    DropTrigger {
        target: ObjectNameDto,
        name: String,
    },
    SaveRoutine {
        target: ObjectNameDto,
        routine: RoutineKindDto,
        replaces: Option<String>,
        definition: String,
    },
    DropRoutine {
        target: ObjectNameDto,
        routine: RoutineKindDto,
        arguments: Option<String>,
    },
}

/// A database or schema change.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum ContainerChangeDto {
    CreateDatabase {
        name: String,
        encoding: Option<String>,
        collation: Option<String>,
    },
    RenameDatabase {
        name: String,
        to: String,
    },
    DropDatabase {
        name: String,
    },
    CreateSchema {
        name: String,
    },
    RenameSchema {
        name: String,
        to: String,
    },
    DropSchema {
        name: String,
        cascade: bool,
    },
}

/// Preview (no token) or run (the preview's token) of one change.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ObjectRequest {
    pub(crate) profile_id: String,
    /// The open database the object lives in (or the change is made from).
    pub(crate) database: String,
    pub(crate) change: ObjectChangeDto,
    pub(crate) confirm_token: Option<String>,
    /// The typed object name, when the preview asked for it.
    pub(crate) confirm_name: Option<String>,
}

/// The statements to review, or the committed result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum ObjectReply {
    Prepared {
        token: String,
        statements: Vec<String>,
        warnings: Vec<String>,
        destructive: bool,
        /// Name to type before running (destructive changes on a
        /// production or confirm-writes connection).
        #[serde(rename = "confirmName")]
        confirm_name: Option<String>,
        /// The statements run in one transaction (else one by one).
        transactional: bool,
    },
    Committed,
}

/// Where a routine lives and which overload, for its definition.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RoutineTarget {
    pub(crate) profile_id: String,
    pub(crate) database: String,
    pub(crate) schema: String,
    pub(crate) name: String,
    pub(crate) routine: RoutineKindDto,
    pub(crate) arguments: Option<String>,
}

impl From<RoutineKindDto> for RoutineKind {
    fn from(kind: RoutineKindDto) -> Self {
        match kind {
            RoutineKindDto::Function => Self::Function,
            RoutineKindDto::Procedure => Self::Procedure,
        }
    }
}

impl From<NewColumnDto> for NewColumn {
    fn from(column: NewColumnDto) -> Self {
        Self {
            name: column.name,
            data_type: column.data_type,
            nullable: column.nullable,
            default: (&column.default).into(),
            comment: column.comment,
            identity: column.identity.into(),
            collation: column.collation,
            charset: column.charset,
        }
    }
}

impl From<NewIndexDto> for NewIndex {
    fn from(index: NewIndexDto) -> Self {
        Self {
            name: index.name,
            columns: index.columns,
            unique: index.unique,
            method: index.method.into(),
            descending: index.descending,
            predicate: index.predicate,
        }
    }
}

impl From<NewTableDto> for NewTable {
    fn from(table: NewTableDto) -> Self {
        Self {
            columns: table.columns.into_iter().map(Into::into).collect(),
            primary_key: table.primary_key,
            indexes: table.indexes.into_iter().map(Into::into).collect(),
            foreign_keys: table.foreign_keys.into_iter().map(Into::into).collect(),
            checks: table.checks.into_iter().map(Into::into).collect(),
            comment: table.comment,
        }
    }
}

impl From<ObjectNameDto> for ObjectName {
    fn from(object: ObjectNameDto) -> Self {
        Self {
            schema: object.schema,
            name: object.name,
        }
    }
}

impl ObjectChangeDto {
    /// The name a guarded destructive change asks to type.
    pub(crate) fn object_name(&self) -> &str {
        match self {
            Self::Relation(change) => change.target_name(),
            Self::Definition(change) => change.object_name(),
            Self::Container(change) => change.object_name(),
        }
    }
}

impl RelationChangeDto {
    fn target_name(&self) -> &str {
        match self {
            Self::CreateTable { target, .. }
            | Self::RenameTable { target, .. }
            | Self::DuplicateTable { target, .. }
            | Self::TruncateTable { target, .. }
            | Self::DropTable { target, .. }
            | Self::SaveView { target, .. }
            | Self::DropView { target, .. } => &target.name,
        }
    }
}

impl DefinitionChangeDto {
    fn object_name(&self) -> &str {
        match self {
            Self::SaveTrigger { target, .. }
            | Self::SaveRoutine { target, .. }
            | Self::DropRoutine { target, .. } => &target.name,
            Self::DropTrigger { name, .. } => name,
        }
    }
}

impl ContainerChangeDto {
    fn object_name(&self) -> &str {
        match self {
            Self::CreateDatabase { name, .. }
            | Self::RenameDatabase { name, .. }
            | Self::DropDatabase { name }
            | Self::CreateSchema { name }
            | Self::RenameSchema { name, .. }
            | Self::DropSchema { name, .. } => name,
        }
    }
}

impl From<ObjectChangeDto> for ObjectChange {
    fn from(change: ObjectChangeDto) -> Self {
        match change {
            ObjectChangeDto::Relation(change) => change.into(),
            ObjectChangeDto::Definition(change) => change.into(),
            ObjectChangeDto::Container(change) => change.into(),
        }
    }
}

impl From<RelationChangeDto> for ObjectChange {
    fn from(change: RelationChangeDto) -> Self {
        use RelationChangeDto as Dto;
        match change {
            Dto::CreateTable { target, definition } => Self::CreateTable {
                table: target.into(),
                definition: definition.into(),
            },
            Dto::RenameTable { target, name } => Self::RenameTable {
                table: target.into(),
                name,
            },
            Dto::DuplicateTable {
                target,
                name,
                with_data,
            } => Self::DuplicateTable {
                table: target.into(),
                name,
                with_data,
            },
            Dto::TruncateTable { target, cascade } => Self::TruncateTable {
                table: target.into(),
                cascade,
            },
            Dto::DropTable { target, cascade } => Self::DropTable {
                table: target.into(),
                cascade,
            },
            Dto::SaveView {
                target,
                query,
                materialized,
                replace,
            } => Self::SaveView {
                view: target.into(),
                query,
                materialized,
                replace,
            },
            Dto::DropView {
                target,
                materialized,
                cascade,
            } => Self::DropView {
                view: target.into(),
                materialized,
                cascade,
            },
        }
    }
}

impl From<DefinitionChangeDto> for ObjectChange {
    fn from(change: DefinitionChangeDto) -> Self {
        use DefinitionChangeDto as Dto;
        match change {
            Dto::SaveTrigger {
                target,
                replaces,
                definition,
            } => Self::SaveTrigger {
                table: target.into(),
                replaces,
                definition,
            },
            Dto::DropTrigger { target, name } => Self::DropTrigger {
                table: target.into(),
                name,
            },
            Dto::SaveRoutine {
                target,
                routine,
                replaces,
                definition,
            } => Self::SaveRoutine {
                routine: target.into(),
                kind: routine.into(),
                replaces,
                definition,
            },
            Dto::DropRoutine {
                target,
                routine,
                arguments,
            } => Self::DropRoutine {
                routine: target.into(),
                kind: routine.into(),
                arguments,
            },
        }
    }
}

/// Database and schema changes.
impl From<ContainerChangeDto> for ObjectChange {
    fn from(change: ContainerChangeDto) -> Self {
        use ContainerChangeDto as Dto;
        match change {
            Dto::CreateDatabase {
                name,
                encoding,
                collation,
            } => Self::CreateDatabase {
                name,
                encoding,
                collation,
            },
            Dto::RenameDatabase { name, to } => Self::RenameDatabase { name, to },
            Dto::DropDatabase { name } => Self::DropDatabase { name },
            Dto::CreateSchema { name } => Self::CreateSchema { name },
            Dto::RenameSchema { name, to } => Self::RenameSchema { name, to },
            Dto::DropSchema { name, cascade } => Self::DropSchema { name, cascade },
        }
    }
}
