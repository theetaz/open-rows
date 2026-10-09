// SPDX-License-Identifier: Apache-2.0
//! `export_start`: resolve the rows, let the user choose the file in the
//! native save panel, then stream in the background.
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use arrow_schema::SchemaRef;
use openrows_core::export::{ExportJob, Progress, export_batches, export_statement};
use openrows_core::script::ScriptCanceller;
use openrows_export::Exporter;
use openrows_results::{ResultStore, StoredBatches};
use openrows_sql::select::{BoundSql, TableRef, all_rows_sql};
use openrows_sql::{Dialect, StatementKind, classify, placeholders, split};
use openrows_types::{QueryId, SessionId};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt as _;

use super::dto::{
    ExportEventDto, ExportReply, ExportRequest, ExportSourceDto, rows, terminal, write_error,
};
use crate::connections::catalog::read_columns;
use crate::connections::drivers::driver;
use crate::connections::{Connections, IpcError, IpcErrorKind};
use crate::queries::Queries;
use crate::tables::commands::{ensure_read, owned_session};

/// Longest suggested file name, in characters.
const MAX_FILE_STEM: usize = 120;

/// Where the rows of an export come from once resolved.
enum Rows {
    /// A result in the store, read lazily (spilled batches decode on demand).
    Stored(SchemaRef, StoredBatches),
    /// A statement streamed from the server on this session.
    Statement(SessionId, BoundSql),
}

/// Everything a started export needs after the command returned.
struct Run {
    export_id: String,
    rows: Rows,
    job: ExportJob,
    path: PathBuf,
    events: Channel<ExportEventDto>,
}

/// Validate the source and options, ask for the destination, then stream
/// the rows to it without sending them to the webview.
///
/// Export only reads, so read-only and confirm-writes connections allow it;
/// a re-run statement must classify as a plain read. `query_cancel(exportId)`
/// cancels; the partial file is deleted.
#[tauri::command]
#[specta::specta]
pub(crate) async fn export_start(
    window: tauri::WebviewWindow,
    app: AppHandle,
    request: ExportRequest,
    on_event: Channel<ExportEventDto>,
) -> Result<ExportReply, IpcError> {
    let connections = app.state::<Connections>();
    let (session, dialect) = session_dialect(&connections, request.source.session_id())?;
    let rows = resolve(&app, (session, dialect), &request.source).await?;
    let format = request.format.to_format(dialect)?;
    let Some(path) = choose_file(&window, &request).await? else {
        return Ok(ExportReply::Dismissed);
    };
    let exporter = Exporter::create(&path, &format).map_err(|error| write_error(&error))?;
    let export_id = QueryId::new().to_string();
    let cancel = ScriptCanceller::default();
    app.state::<Queries>().track(&export_id, cancel.clone());
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let run = Run {
        export_id: export_id.clone(),
        rows,
        job: ExportJob {
            exporter,
            cancel,
            progress: progress(on_event.clone()),
        },
        path,
        events: on_event,
    };
    tauri::async_runtime::spawn(execute(app.clone(), run));
    Ok(ExportReply::Started {
        export_id,
        file_name,
    })
}

/// The open session and the dialect of its saved connection's engine.
fn session_dialect(
    connections: &Connections,
    session_id: &str,
) -> Result<(SessionId, Dialect), IpcError> {
    let session =
        SessionId::from_str(session_id).map_err(|_| IpcError::invalid("Invalid session id"))?;
    let owner = connections
        .owners
        .owner(session)
        .ok_or_else(|| IpcError::new(IpcErrorKind::NotFound, "The session is not open"))?;
    let profile = connections.profiles.get(owner.profile)?;
    Ok((session, driver(profile.driver).dialect()))
}

async fn resolve(
    app: &AppHandle,
    (session, dialect): (SessionId, Dialect),
    source: &ExportSourceDto,
) -> Result<Rows, IpcError> {
    match source {
        ExportSourceDto::Result { result_id, .. } => app
            .state::<Arc<ResultStore>>()
            .batches(result_id)
            .map(|(schema, batches)| Rows::Stored(schema, batches))
            .ok_or_else(|| {
                IpcError::new(
                    IpcErrorKind::NotFound,
                    "This result is no longer available; run the query again",
                )
            }),
        ExportSourceDto::Query { sql, .. } => {
            Ok(Rows::Statement(session, read_statement(sql, dialect)?))
        }
        ExportSourceDto::Table { session_id, target } => {
            let connections = app.state::<Connections>();
            let session = owned_session(&connections, session_id, target)?;
            let columns = read_columns(
                &connections,
                (&target.profile_id, &target.database),
                (target.schema.clone(), target.table.clone()),
            )
            .await?;
            let known: Vec<String> = columns.into_iter().map(|column| column.name).collect();
            let table = TableRef {
                schema: target.schema.clone(),
                name: target.table.clone(),
            };
            let bound = all_rows_sql(dialect, &table, &known).map_err(|_| {
                IpcError::new(
                    IpcErrorKind::NotFound,
                    format!("{}.{} was not found", target.schema, target.table),
                )
            })?;
            ensure_read(&bound, dialect)?;
            Ok(Rows::Statement(session, bound))
        }
    }
}

/// One plain read statement to run again; anything else is refused.
fn read_statement(sql: &str, dialect: Dialect) -> Result<BoundSql, IpcError> {
    let refused =
        || IpcError::invalid("Only a single read-only statement can be run again for export");
    let statements = split(sql, dialect);
    let [statement] = statements.as_slice() else {
        return Err(refused());
    };
    let text = statement.text(sql);
    let class = classify(text, dialect);
    if class.kind != StatementKind::Read || class.dangerous {
        return Err(refused());
    }
    // Bound values exist only in the run request; export the received rows instead.
    if !placeholders(text, dialect).is_empty() {
        return Err(IpcError::invalid(
            "A statement with parameters cannot be run again for export; export the received rows",
        ));
    }
    Ok(BoundSql {
        sql: text.to_owned(),
        params: Vec::new(),
    })
}

/// The native save panel; `None` when the user cancelled it.
async fn choose_file(
    window: &tauri::WebviewWindow,
    request: &ExportRequest,
) -> Result<Option<PathBuf>, IpcError> {
    let (extension, label) = request.format.extension();
    let (sender, receiver) = tokio::sync::oneshot::channel();
    window
        .dialog()
        .file()
        .set_parent(window)
        .set_title("Export")
        .set_file_name(format!("{}.{extension}", file_stem(&request.file_name)))
        .add_filter(label, &[extension])
        .save_file(move |picked| {
            // The receiver only disappears when the command was dropped.
            let _ = sender.send(picked);
        });
    let picked = receiver
        .await
        .map_err(|_| IpcError::new(IpcErrorKind::Internal, "The save panel closed unexpectedly"))?;
    picked
        .map(|file| {
            file.into_path()
                .map_err(|error| IpcError::invalid(error.to_string()))
        })
        .transpose()
}

/// A safe suggested file name: no path separators, never empty.
fn file_stem(name: &str) -> String {
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

fn progress(events: Channel<ExportEventDto>) -> Progress {
    Box::new(move |count| {
        // A closed channel means the window is gone; the export still finishes.
        let _ = events.send(ExportEventDto::Progress { rows: rows(count) });
    })
}

async fn execute(app: AppHandle, run: Run) {
    let outcome = match run.rows {
        Rows::Stored(schema, batches) => export_batches((schema, batches), run.job).await,
        Rows::Statement(session, statement) => {
            let connections = app.state::<Connections>();
            export_statement(&connections.sessions, session, statement, run.job).await
        }
    };
    let _ = run
        .events
        .send(terminal(outcome, run.path.display().to_string()));
    app.state::<Queries>().forget(&run.export_id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_one_plain_read_can_be_run_again() -> Result<(), IpcError> {
        let bound = read_statement("  SELECT * FROM t ;  ", Dialect::Postgres)?;
        assert_eq!(bound.sql, "SELECT * FROM t");
        assert!(bound.params.is_empty());
        for refused in [
            "DELETE FROM t",
            "SELECT 1; SELECT 2",
            "UPDATE t SET a = 1",
            "",
            "WITH d AS (DELETE FROM t RETURNING *) SELECT * FROM d",
            "SELECT * FROM t WHERE id = ?",
        ] {
            assert!(
                read_statement(refused, Dialect::MySql).is_err(),
                "{refused}"
            );
        }
        Ok(())
    }

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
