// SPDX-License-Identifier: Apache-2.0
//! Secret-free preview rows and explicit import decisions.
use super::super::dto::ProfileDto;
use openrows_store::profile::exchange::{Conflict, ImportChoice, ImportDecision};
use serde::{Deserialize, Serialize};
use specta::Type;
/// Public format selected by the user; open files use the native picker.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ImportSource {
    OpenFile,
    Project { root: String },
    Pgpass,
    PgService,
    MyCnf,
}
/// Classification displayed before any profile or keychain write.
#[derive(Clone, Copy, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ConnectionConflictDto {
    New,
    SameName,
    Identical,
}
impl From<Conflict> for ConnectionConflictDto {
    fn from(conflict: Conflict) -> Self {
        match conflict {
            Conflict::New => Self::New,
            Conflict::SameName(_) => Self::SameName,
            Conflict::Identical => Self::Identical,
        }
    }
}
/// One preview row, never carrying an imported password.
#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportRow {
    pub(crate) profile: ProfileDto,
    pub(crate) conflict: ConnectionConflictDto,
    pub(crate) has_password: bool,
}
/// A single-use core-owned preview token and its safe display data.
#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportPreview {
    pub(crate) token: String,
    pub(crate) rows: Vec<ImportRow>,
    pub(crate) warnings: Vec<String>,
}
/// Chosen conflict action.
#[derive(Clone, Copy, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ImportChoiceDto {
    Skip,
    Replace,
    KeepBoth,
}
/// User-confirmed action and optional keychain password save.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportDecisionDto {
    choice: ImportChoiceDto,
    save_password: bool,
}
impl From<&ImportDecisionDto> for ImportDecision {
    fn from(decision: &ImportDecisionDto) -> Self {
        Self {
            choice: match decision.choice {
                ImportChoiceDto::Skip => ImportChoice::Skip,
                ImportChoiceDto::Replace => ImportChoice::Replace,
                ImportChoiceDto::KeepBoth => ImportChoice::KeepBoth,
            },
            save_password: decision.save_password,
        }
    }
}
