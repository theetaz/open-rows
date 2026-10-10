// SPDX-License-Identifier: Apache-2.0
//! `import_pick_file`, `import_preview`, `import_start` and `import_cancel`.
use std::path::{Path, PathBuf};

use openrows_driver_api::{Connection, catalog};
use openrows_import::{
    CancellationToken, FeedSource, ImportError, Inspection, ParseOptions, SampleLimits,
    TypeCategory, inspect,
};
use openrows_sql::Dialect;
use openrows_types::QueryId;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt as _;

use super::Imports;
use super::dto::{
    ImportCellDto, ImportColumnDto, ImportEventDto, ImportFileReply, ImportModeDto,
    ImportOptionsDto, ImportPreviewDto, ImportPreviewRequest, ImportReply, ImportRequest,
    RowErrorDto, count,
};
use super::plan::{batch_rows, plan};
use super::run::{Job, execute};
use crate::connections::drivers::{driver, profile_driver};
use crate::connections::profiles::profile_id;
use crate::connections::{Connections, IpcError, IpcErrorKind, route, target};
use crate::tables::TableTarget;
use crate::tables::commands::owned_session;

/// Rows shown in the preview and read for type inference.
const PREVIEW: SampleLimits = SampleLimits {
    preview: 50,
    sample: 1_000,
};
/// Extensions offered in the open panel.
const EXTENSIONS: [&str; 6] = ["csv", "tsv", "txt", "json", "ndjson", "jsonl"];

/// A file failure as a user-facing error.
pub(super) fn file_error(error: &ImportError) -> IpcError {
    let kind = match error {
        ImportError::Io(_) => IpcErrorKind::Storage,
        _ => IpcErrorKind::Invalid,
    };
    let message = error.to_string();
    let mut chars = message.chars();
    let message = chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    });
    IpcError::new(kind, message)
}

fn file_path(imports: &Imports, file_id: &str) -> Result<PathBuf, IpcError> {
    imports
        .path(file_id)
        .ok_or_else(|| IpcError::new(IpcErrorKind::NotFound, "Choose the file to import again"))
}

/// Inspect on a blocking thread.
async fn inspected(path: PathBuf, options: Option<ParseOptions>) -> Result<Inspection, IpcError> {
    tauri::async_runtime::spawn_blocking(move || inspect(&path, options, PREVIEW))
        .await
        .map_err(|_| IpcError::new(IpcErrorKind::Internal, "Reading the file stopped"))?
        .map_err(|error| file_error(&error))
}

/// Show the native open panel; the chosen path stays in the core.
#[tauri::command]
#[specta::specta]
pub(crate) async fn import_pick_file(
    window: tauri::WebviewWindow,
    imports: State<'_, Imports>,
) -> Result<ImportFileReply, IpcError> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    window
        .dialog()
        .file()
        .set_parent(&window)
        .set_title("Import")
        .add_filter("CSV, TSV, JSON or NDJSON", &EXTENSIONS)
        .pick_file(move |picked| {
            // The receiver only disappears when the command was dropped.
            let _ = sender.send(picked);
        });
    let picked = receiver
        .await
        .map_err(|_| IpcError::new(IpcErrorKind::Internal, "The open panel closed unexpectedly"))?;
    let Some(file) = picked else {
        return Ok(ImportFileReply::Dismissed);
    };
    let path = file
        .into_path()
        .map_err(|error| IpcError::invalid(error.to_string()))?;
    let size = std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
    let file_id = QueryId::new().to_string();
    let file_name = file_name(&path);
    imports.remember(&file_id, path);
    Ok(ImportFileReply::Picked {
        file_id,
        file_name,
        size_bytes: count(size),
    })
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Detect (or apply) parse options and preview the file's columns and rows.
#[tauri::command]
#[specta::specta]
pub(crate) async fn import_preview(
    connections: State<'_, Connections>,
    imports: State<'_, Imports>,
    request: ImportPreviewRequest,
) -> Result<ImportPreviewDto, IpcError> {
    let path = file_path(&imports, &request.file_id)?;
    let dialect = profile_driver(&connections, &request.profile_id)?.dialect();
    let options = request
        .options
        .as_ref()
        .map(ImportOptionsDto::to_options)
        .transpose()?;
    let inspection = inspected(path, options).await?;
    Ok(preview(&inspection, dialect, &request.types))
}

/// The preview DTO; cells `types` would reject are listed as invalid.
fn preview(
    inspection: &Inspection,
    dialect: Dialect,
    types: &[Option<String>],
) -> ImportPreviewDto {
    let categories: Vec<Option<TypeCategory>> = (0..inspection.columns.len())
        .map(|index| {
            types
                .get(index)
                .cloned()
                .flatten()
                .map(|t| TypeCategory::of_type(&t))
        })
        .collect();
    let mut invalid = Vec::new();
    for (row, record) in (0_u32..).zip(&inspection.rows) {
        for ((column, cell), category) in (0_u32..).zip(&record.cells).zip(&categories) {
            let rejected = matches!((cell, category), (Some(value), Some(category)) if !category.accepts(value.trim()));
            if rejected {
                invalid.push(ImportCellDto { row, column });
            }
        }
    }
    ImportPreviewDto {
        options: ImportOptionsDto::from(&inspection.options),
        columns: inspection
            .columns
            .iter()
            .map(|column| ImportColumnDto {
                name: column.name.clone(),
                category: column.stats.category().into(),
                nullable: column.stats.nullable,
                suggested_type: column
                    .stats
                    .category()
                    .default_type(dialect, column.stats.longest)
                    .to_owned(),
            })
            .collect(),
        rows: inspection
            .rows
            .iter()
            .map(|record| record.cells.clone())
            .collect(),
        lines: inspection
            .rows
            .iter()
            .map(|record| count(record.line))
            .collect(),
        invalid,
        errors: inspection.errors.iter().map(RowErrorDto::from).collect(),
        sampled: u32::try_from(inspection.sampled).unwrap_or(u32::MAX),
    }
}

/// Imports write: read-only connections refuse them (dry runs only read).
pub(super) fn ensure_writable(read_only: bool) -> Result<(), IpcError> {
    if read_only {
        return Err(IpcError::invalid(
            "This connection is read-only: imports are disabled",
        ));
    }
    Ok(())
}

/// A dedicated connection to the target's database.
async fn connect(
    state: &Connections,
    target: &TableTarget,
) -> Result<Box<dyn Connection>, IpcError> {
    let destination = route::profile_target(state, &target.profile_id)
        .await?
        .with_database(&target.database)
        .with_application_suffix("import");
    target::connect(&destination)
        .await
        .map(|connected| connected.connection)
        .map_err(|error| IpcError::from(&error))
}

/// The table's columns and types; empty for a table the import creates.
async fn existing_columns(
    connection: &mut dyn Connection,
    request: &ImportRequest,
) -> Result<Vec<(String, String)>, IpcError> {
    if request.create.is_some() {
        return Ok(Vec::new());
    }
    let target = &request.target;
    let columns = catalog(connection)
        .map_err(|error| IpcError::from(&error))?
        .table_columns(&target.schema, &target.table)
        .await
        .map_err(|error| IpcError::from(&error))?;
    if columns.is_empty() {
        return Err(IpcError::new(
            IpcErrorKind::NotFound,
            format!("{}.{} was not found", target.schema, target.table),
        ));
    }
    Ok(columns.into_iter().map(|c| (c.name, c.data_type)).collect())
}

/// Check the request, connect and start the dry run or import in the
/// background; events follow on `on_event`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn import_start(
    app: AppHandle,
    request: ImportRequest,
    on_event: Channel<ImportEventDto>,
) -> Result<ImportReply, IpcError> {
    let connections = app.state::<Connections>();
    owned_session(&connections, &request.session_id, &request.target)?;
    let profile = connections
        .profiles
        .get(profile_id(&request.target.profile_id)?)?;
    if request.mode == ImportModeDto::Import {
        ensure_writable(profile.safety.read_only())?;
    }
    let dialect = driver(profile.driver).dialect();
    let path = file_path(&app.state::<Imports>(), &request.file_id)?;
    let options = request.options.to_options()?;
    let inspection = inspected(path.clone(), Some(options.clone())).await?;
    let columns: Vec<String> = inspection.columns.into_iter().map(|c| c.name).collect();
    let mut connection = connect(&connections, &request.target).await?;
    let planned = match existing_columns(connection.as_mut(), &request).await {
        Ok(existing) => plan(&request, dialect, columns.len(), &existing),
        Err(error) => Err(error),
    };
    let (plan, mapper) = match planned {
        Ok(planned) => planned,
        Err(error) => {
            // The plan error matters more than a failing close.
            let _ = connection.close().await;
            return Err(error);
        }
    };
    let approval = crate::security::confirm_batch(
        &app,
        &profile,
        super::plan::safety_statements(&plan)?,
        &request.target.database,
    )
    .await;
    if let Err(error) = approval {
        let _ = connection.close().await;
        return Err(error);
    }
    let import_id = QueryId::new().to_string();
    let cancel = CancellationToken::new();
    app.state::<Imports>().track(&import_id, cancel.clone());
    let size = std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
    let job = Job {
        import_id: import_id.clone(),
        connection,
        plan,
        feed: FeedSource {
            path,
            options,
            columns,
            mapper,
            batch_rows: batch_rows(request.batch_size),
        },
        size,
        cancel,
        events: on_event,
        target: request.target,
    };
    tauri::async_runtime::spawn(execute(app.clone(), job));
    Ok(ImportReply { import_id })
}

/// Cancel a running dry run or import; uncommitted work is rolled back.
#[tauri::command]
#[specta::specta]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri deserialises command arguments into owned values"
)]
pub(crate) fn import_cancel(imports: State<'_, Imports>, import_id: String) {
    imports.cancel(&import_id);
}

#[cfg(test)]
mod tests {
    use super::*;
    use openrows_import::{Record, SourceColumn};

    #[test]
    fn previews_mark_cells_the_target_type_rejects() {
        let mut stats = openrows_import::ColumnStats::default();
        stats.observe(Some("x"));
        let inspection = Inspection {
            options: ParseOptions::default(),
            columns: vec![
                SourceColumn {
                    name: "id".to_owned(),
                    stats,
                },
                SourceColumn {
                    name: "name".to_owned(),
                    stats,
                },
            ],
            rows: vec![
                Record {
                    line: 2,
                    cells: vec![Some("1".to_owned()), Some("a".to_owned())],
                },
                Record {
                    line: 3,
                    cells: vec![Some("x".to_owned()), None],
                },
            ],
            errors: Vec::new(),
            sampled: 2,
        };
        let types = [Some("integer".to_owned()), Some("integer".to_owned())];
        let dto = preview(&inspection, Dialect::MySql, &types);
        assert_eq!(
            dto.invalid,
            [
                ImportCellDto { row: 0, column: 1 },
                ImportCellDto { row: 1, column: 0 }
            ]
        );
        assert_eq!(dto.columns[0].suggested_type, "VARCHAR(255)");
        assert_eq!(dto.lines, [2.0, 3.0]);
        assert!(preview(&inspection, Dialect::MySql, &[]).invalid.is_empty());
    }

    #[test]
    fn read_only_connections_refuse_imports() {
        assert!(ensure_writable(true).is_err());
        assert!(ensure_writable(false).is_ok());
        let error = file_error(&ImportError::NoColumns);
        assert_eq!(error.message, "The file has no columns");
        assert_eq!(error.kind, IpcErrorKind::Invalid);
    }
}
