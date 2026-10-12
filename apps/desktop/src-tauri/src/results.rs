// SPDX-License-Identifier: Apache-2.0
use openrows_results::{CellInfo, CellKind, ResultStore, Window, WindowError};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WindowRequest {
    result_id: String,
    row_start: u32,
    row_count: u32,
    col_start: u32,
    col_count: u32,
}

/// One cell of a stored result, by its row and column index.
#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CellRequest {
    result_id: String,
    row: u32,
    column: u32,
}

impl CellRequest {
    fn address(&self) -> (&str, usize, usize) {
        (
            self.result_id.as_str(),
            self.row as usize,
            self.column as usize,
        )
    }
}

/// What a cell holds, for the value inspector.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CellKindDto {
    Null,
    Text,
    Binary,
    Other,
}

/// Kind and exact byte length of a cell; the value itself is read separately.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CellInfoDto {
    kind: CellKindDto,
    /// Bytes of the exact value (UTF-8 for text); zero unless text or binary.
    byte_len: f64,
}

impl From<CellInfo> for CellInfoDto {
    #[expect(
        clippy::cast_precision_loss,
        reason = "byte lengths stay far below 2^53 (JS exact integers)"
    )]
    fn from(info: CellInfo) -> Self {
        Self {
            kind: match info.kind {
                CellKind::Null => CellKindDto::Null,
                CellKind::Text => CellKindDto::Text,
                CellKind::Binary => CellKindDto::Binary,
                CellKind::Other => CellKindDto::Other,
            },
            byte_len: info.byte_len as f64,
        }
    }
}

#[derive(Debug, Serialize, Type)]
#[serde(tag = "kind", content = "message", rename_all = "camelCase")]
pub(crate) enum ResultError {
    UnknownResult(String),
    OutOfRange,
    NotBytes,
    Encoding(String),
    Unavailable(String),
}

impl From<WindowError> for ResultError {
    fn from(error: WindowError) -> Self {
        match error {
            WindowError::UnknownResult(id) => Self::UnknownResult(id),
            WindowError::OutOfRange => Self::OutOfRange,
            WindowError::NotBytes => Self::NotBytes,
            WindowError::Spill(error) => Self::Unavailable(error.to_string()),
            other => Self::Encoding(other.to_string()),
        }
    }
}

// Response intentionally bypasses Specta's JSON return model. The request and
// error types are exported; @openrows/ipc validates the ArrayBuffer at runtime.
#[tauri::command]
pub(crate) async fn result_window(
    request: WindowRequest,
    store: tauri::State<'_, Arc<ResultStore>>,
) -> Result<tauri::ipc::Response, ResultError> {
    #[cfg(all(feature = "test-automation", debug_assertions, unix))]
    crate::automation::record_window(serde_json::json!({
        "resultId": request.result_id, "rowStart": request.row_start, "rowCount": request.row_count,
        "colStart": request.col_start, "colCount": request.col_count
    }));
    let store = Arc::clone(store.inner());
    tauri::async_runtime::spawn_blocking(move || {
        let window = Window {
            row_start: request.row_start as usize,
            row_count: request.row_count as usize,
            col_start: request.col_start as usize,
            col_count: request.col_count as usize,
        };
        store
            .window(&request.result_id, window)
            .map(tauri::ipc::Response::new)
            .map_err(Into::into)
    })
    .await
    .map_err(|error| ResultError::Unavailable(error.to_string()))?
}

/// Kind and byte length of one cell (value inspector); nothing is copied.
#[tauri::command]
#[specta::specta]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri deserialises command arguments into owned values"
)]
pub(crate) fn result_cell_info(
    request: CellRequest,
    store: tauri::State<'_, Arc<ResultStore>>,
) -> Result<CellInfoDto, ResultError> {
    let (id, row, column) = request.address();
    store
        .cell_info(id, row, column)
        .map(CellInfoDto::from)
        .map_err(Into::into)
}

/// The exact value of a text (UTF-8) or binary cell as raw bytes; the value
/// inspector loads it on demand, so a large cell never sits in the grid.
// Raw bytes like `result_window`; @openrows/ipc normalises the reply.
#[tauri::command]
pub(crate) async fn result_cell_value(
    request: CellRequest,
    store: tauri::State<'_, Arc<ResultStore>>,
) -> Result<tauri::ipc::Response, ResultError> {
    let store = Arc::clone(store.inner());
    tauri::async_runtime::spawn_blocking(move || {
        let (id, row, column) = request.address();
        store
            .cell_bytes(id, row, column)
            .map(tauri::ipc::Response::new)
            .map_err(Into::into)
    })
    .await
    .map_err(|error| ResultError::Unavailable(error.to_string()))?
}

#[cfg(test)]
mod tests {
    use tauri::ipc::{InvokeResponseBody, IpcResponse};

    #[test]
    fn response_is_raw_not_json() -> tauri::Result<()> {
        assert!(matches!(
            tauri::ipc::Response::new(vec![1, 2, 3]).body()?,
            InvokeResponseBody::Raw(_)
        ));
        Ok(())
    }

    #[test]
    fn cell_kinds_map_onto_the_wire_names() -> Result<(), serde_json::Error> {
        let info = super::CellInfoDto::from(openrows_results::CellInfo {
            kind: openrows_results::CellKind::Binary,
            byte_len: 3,
        });
        let json = serde_json::to_value(info)?;
        assert_eq!(json["kind"], "binary");
        assert_eq!(json["byteLen"], 3.0);
        Ok(())
    }
}
