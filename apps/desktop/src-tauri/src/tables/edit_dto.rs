// SPDX-License-Identifier: Apache-2.0
//! The row-edit IPC contract. Snapshots remain in memory, not in query history.
use super::dto::{TableColumnDto, TableTarget};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EditInfoDto {
    pub(crate) key: Vec<String>,
    pub(crate) protected: Vec<String>,
    pub(crate) columns: Vec<TableColumnDto>,
    pub(crate) read_only: bool,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum EditKindDto {
    Update,
    Insert,
    Delete,
}
#[derive(Clone, Debug, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RowEditDto {
    pub(crate) kind: EditKindDto,
    pub(crate) original: BTreeMap<String, Option<String>>,
    pub(crate) values: BTreeMap<String, Option<String>>,
}
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EditRequest {
    pub(crate) session_id: String,
    pub(crate) target: TableTarget,
    pub(crate) changes: Vec<RowEditDto>,
    pub(crate) confirm_token: Option<String>,
}
#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EditStatementDto {
    pub(crate) sql: String,
    pub(crate) params: Vec<Option<String>>,
}
#[derive(Clone, Debug, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum EditReply {
    Prepared {
        token: String,
        statements: Vec<EditStatementDto>,
    },
    Committed,
}
