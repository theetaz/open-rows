// SPDX-License-Identifier: Apache-2.0
//! The row-edit IPC contract. Snapshots remain in memory, not in query history.
use super::dto::{TableColumnDto, TableTarget};
use openrows_sql::edit::{CellValue, Edit, EditKind};
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
/// A staged field value. `default` is the column DEFAULT, never the text `'DEFAULT'`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum CellValueDto {
    Null,
    Default,
    Text { value: String },
}
impl From<CellValueDto> for CellValue {
    fn from(value: CellValueDto) -> Self {
        match value {
            CellValueDto::Null => Self::Null,
            CellValueDto::Default => Self::Default,
            CellValueDto::Text { value } => Self::Text(value),
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RowEditDto {
    pub(crate) kind: EditKindDto,
    /// Exact original text; `null` is SQL NULL.
    pub(crate) original: BTreeMap<String, Option<String>>,
    pub(crate) values: BTreeMap<String, CellValueDto>,
}
impl From<&RowEditDto> for Edit {
    fn from(change: &RowEditDto) -> Self {
        Self {
            kind: match change.kind {
                EditKindDto::Update => EditKind::Update,
                EditKindDto::Insert => EditKind::Insert,
                EditKindDto::Delete => EditKind::Delete,
            },
            original: change.original.clone(),
            values: change
                .values
                .iter()
                .map(|(name, value)| (name.clone(), value.clone().into()))
                .collect(),
        }
    }
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
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn staged_values_are_tagged_so_default_is_not_text() -> Result<(), serde_json::Error> {
        let values: Vec<CellValueDto> = serde_json::from_str(
            r#"[{"kind":"null"},{"kind":"default"},{"kind":"text","value":"DEFAULT"},{"kind":"text","value":""}]"#,
        )?;
        let values: Vec<CellValue> = values.into_iter().map(Into::into).collect();
        assert_eq!(
            values,
            [
                CellValue::Null,
                CellValue::Default,
                CellValue::Text("DEFAULT".into()),
                CellValue::Text(String::new()),
            ]
        );
        assert!(serde_json::from_str::<CellValueDto>(r#""DEFAULT""#).is_err());
        assert!(serde_json::from_str::<CellValueDto>(r#"{"kind":"text"}"#).is_err());
        Ok(())
    }
}
