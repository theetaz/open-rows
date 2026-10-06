// SPDX-License-Identifier: Apache-2.0
use openrows_results::{ResultStore, Window, WindowError};
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

#[derive(Debug, Serialize, Type)]
#[serde(tag = "kind", content = "message", rename_all = "camelCase")]
pub(crate) enum ResultError {
    UnknownResult(String),
    OutOfRange,
    Encoding(String),
    Unavailable(String),
}

impl From<WindowError> for ResultError {
    fn from(error: WindowError) -> Self {
        match error {
            WindowError::UnknownResult(id) => Self::UnknownResult(id),
            WindowError::OutOfRange => Self::OutOfRange,
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
}
