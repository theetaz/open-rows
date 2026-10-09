// SPDX-License-Identifier: Apache-2.0
//! `export_start`: resolve the rows, let the user choose the file (or the
//! folder for one file per table) in a native panel, then export in the
//! background as a job.
use std::path::PathBuf;

use openrows_core::script::ScriptCanceller;
use openrows_export::Exporter;
use openrows_types::QueryId;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt as _;

use super::dto::{ExportEventDto, ExportReply, ExportRequest, ExportSourceDto, write_error};
use super::format_dto::ExportFormatDto;
use super::plan::{Plan, resolve};
use super::run::{Destination, Job, execute};
use crate::connections::{Connections, IpcError, IpcErrorKind};
use crate::jobs::job_scope;
use crate::queries::Queries;

/// Longest suggested file name, in characters.
const MAX_FILE_STEM: usize = 120;

/// Validate the source and options, ask for the destination, then stream
/// the rows to it without sending them to the webview.
///
/// Export only reads, so read-only and confirm-writes connections allow it;
/// a re-run statement must classify as a plain read. Statements run on a
/// session of the export's own. `query_cancel(exportId)` cancels; partial
/// files are deleted.
#[tauri::command]
#[specta::specta]
pub(crate) async fn export_start(
    window: tauri::WebviewWindow,
    app: AppHandle,
    request: ExportRequest,
    on_event: Channel<ExportEventDto>,
) -> Result<ExportReply, IpcError> {
    let request = with_workbook_only_for_excel(request);
    let scope = job_scope(&app.state::<Connections>(), request.source.session_id())?;
    let name = file_stem(&request.file_name);
    // Validates the options before any panel shows.
    let format = request.format.to_format(scope.dialect(), &name)?;
    let plan = resolve(&app, &scope, &request.source).await?;
    let destination = match &plan {
        Plan::PerTable(_) => match choose_folder(&window).await? {
            Some(folder) => Destination::Folder(folder),
            None => return Ok(ExportReply::Dismissed),
        },
        Plan::Stored { .. } | Plan::Statements(_) => {
            let Some(path) = choose_file(&window, (&request.format, &name)).await? else {
                return Ok(ExportReply::Dismissed);
            };
            let exporter = Exporter::create(&path, &format).map_err(|error| write_error(&error))?;
            Destination::File(path, exporter)
        }
    };
    let (Destination::File(path, _) | Destination::Folder(path)) = &destination;
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let export_id = QueryId::new().to_string();
    let cancel = ScriptCanceller::default();
    app.state::<Queries>().track(&export_id, cancel.clone());
    let job = Job {
        export_id: export_id.clone(),
        scope,
        plan,
        destination,
        format: request.format,
        cancel,
        events: on_event,
    };
    tauri::async_runtime::spawn(execute(app.clone(), job));
    Ok(ExportReply::Started {
        export_id,
        file_name,
    })
}

/// A sheet per table needs a workbook format; others write a folder.
fn with_workbook_only_for_excel(mut request: ExportRequest) -> ExportRequest {
    let excel = matches!(request.format, ExportFormatDto::Xlsx { .. });
    if let ExportSourceDto::Tables { workbook, .. } = &mut request.source {
        *workbook &= excel;
    }
    request
}

/// The native save panel; `None` when the user cancelled it.
async fn choose_file(
    window: &tauri::WebviewWindow,
    (format, name): (&ExportFormatDto, &str),
) -> Result<Option<PathBuf>, IpcError> {
    let (extension, label) = format.extension();
    let (sender, receiver) = tokio::sync::oneshot::channel();
    window
        .dialog()
        .file()
        .set_parent(window)
        .set_title("Export")
        .set_file_name(format!("{name}.{extension}"))
        .add_filter(label, &[extension])
        .save_file(move |picked| {
            // The receiver only disappears when the command was dropped.
            let _ = sender.send(picked);
        });
    picked_path(receiver.await)
}

/// The native folder panel for one file per table.
async fn choose_folder(window: &tauri::WebviewWindow) -> Result<Option<PathBuf>, IpcError> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    window
        .dialog()
        .file()
        .set_parent(window)
        .set_title("Export Tables to Folder")
        .set_can_create_directories(true)
        .pick_folder(move |picked| {
            // The receiver only disappears when the command was dropped.
            let _ = sender.send(picked);
        });
    picked_path(receiver.await)
}

fn picked_path(
    picked: Result<Option<tauri_plugin_dialog::FilePath>, tokio::sync::oneshot::error::RecvError>,
) -> Result<Option<PathBuf>, IpcError> {
    let picked = picked
        .map_err(|_| IpcError::new(IpcErrorKind::Internal, "The panel closed unexpectedly"))?;
    picked
        .map(|file| {
            file.into_path()
                .map_err(|error| IpcError::invalid(error.to_string()))
        })
        .transpose()
}

/// A safe file name: no path separators, never empty.
pub(super) fn file_stem(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':' | '\0') {
                '_'
            } else {
                c
            }
        })
        .take(MAX_FILE_STEM)
        .collect();
    let trimmed = cleaned.trim().trim_start_matches('.');
    if trimmed.is_empty() {
        "export".to_owned()
    } else {
        trimmed.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggested_names_cannot_escape_the_chosen_folder() {
        assert_eq!(file_stem("users"), "users");
        assert_eq!(file_stem("../../etc/passwd"), "_.._etc_passwd");
        assert_eq!(file_stem(" a:b\\c "), "a_b_c");
        assert_eq!(file_stem(".hidden"), "hidden");
        assert_eq!(file_stem("   "), "export");
        assert_eq!(file_stem(&"x".repeat(500)).len(), MAX_FILE_STEM);
    }
}
