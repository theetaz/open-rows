// SPDX-License-Identifier: Apache-2.0
//! Column numbering, constraints and their staged additions in the
//! structure-editing IPC contract.
use openrows_driver_api::schema::{
    ConstraintDetail, ConstraintKind, ForeignTarget, Identity, NewCheck, NewForeignKey,
    ReferentialAction,
};
use serde::{Deserialize, Serialize};
use specta::Type;

/// How a column numbers new rows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum IdentityDto {
    #[default]
    None,
    Always,
    ByDefault,
    AutoIncrement,
}

/// `ON DELETE` / `ON UPDATE` action of a foreign key.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ReferentialActionDto {
    #[default]
    NoAction,
    Restrict,
    Cascade,
    SetNull,
    SetDefault,
}

/// The referenced table and columns; an empty schema is the table's own.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ForeignTargetDto {
    pub(crate) schema: String,
    pub(crate) table: String,
    pub(crate) columns: Vec<String>,
}

/// A foreign key to add.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NewForeignKeyDto {
    pub(crate) name: Option<String>,
    pub(crate) columns: Vec<String>,
    pub(crate) target: ForeignTargetDto,
    pub(crate) on_delete: ReferentialActionDto,
    pub(crate) on_update: ReferentialActionDto,
}

/// A check constraint to add.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NewCheckDto {
    pub(crate) name: Option<String>,
    pub(crate) expression: String,
}

/// Kind of an existing constraint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ConstraintKindDto {
    Primary,
    Unique,
    Check,
    Foreign,
    Exclusion,
}

/// One existing constraint of the table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConstraintDto {
    pub(crate) name: String,
    pub(crate) kind: ConstraintKindDto,
    pub(crate) definition: String,
    pub(crate) columns: Vec<String>,
    pub(crate) references: Option<ForeignTargetDto>,
}

impl From<IdentityDto> for Identity {
    fn from(identity: IdentityDto) -> Self {
        match identity {
            IdentityDto::None => Self::None,
            IdentityDto::Always => Self::Always,
            IdentityDto::ByDefault => Self::ByDefault,
            IdentityDto::AutoIncrement => Self::AutoIncrement,
        }
    }
}

impl From<ReferentialActionDto> for ReferentialAction {
    fn from(action: ReferentialActionDto) -> Self {
        match action {
            ReferentialActionDto::NoAction => Self::NoAction,
            ReferentialActionDto::Restrict => Self::Restrict,
            ReferentialActionDto::Cascade => Self::Cascade,
            ReferentialActionDto::SetNull => Self::SetNull,
            ReferentialActionDto::SetDefault => Self::SetDefault,
        }
    }
}

impl From<NewForeignKeyDto> for NewForeignKey {
    fn from(key: NewForeignKeyDto) -> Self {
        Self {
            name: key.name,
            columns: key.columns,
            target: ForeignTarget {
                schema: key.target.schema,
                table: key.target.table,
                columns: key.target.columns,
            },
            on_delete: key.on_delete.into(),
            on_update: key.on_update.into(),
        }
    }
}

impl From<NewCheckDto> for NewCheck {
    fn from(check: NewCheckDto) -> Self {
        Self {
            name: check.name,
            expression: check.expression,
        }
    }
}

impl From<&ConstraintDetail> for ConstraintDto {
    fn from(constraint: &ConstraintDetail) -> Self {
        Self {
            name: constraint.name.clone(),
            kind: match constraint.kind {
                ConstraintKind::Primary => ConstraintKindDto::Primary,
                ConstraintKind::Unique => ConstraintKindDto::Unique,
                ConstraintKind::Check => ConstraintKindDto::Check,
                ConstraintKind::Foreign => ConstraintKindDto::Foreign,
                ConstraintKind::Exclusion => ConstraintKindDto::Exclusion,
            },
            definition: constraint.definition.clone(),
            columns: constraint.columns.clone(),
            references: constraint
                .references
                .as_ref()
                .map(|target| ForeignTargetDto {
                    schema: target.schema.clone(),
                    table: target.table.clone(),
                    columns: target.columns.clone(),
                }),
        }
    }
}
