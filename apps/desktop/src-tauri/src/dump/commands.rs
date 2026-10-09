// SPDX-License-Identifier: Apache-2.0
//! `dump_tools`, `dump_context`, the backup commands and `dump_cancel`.
use std::path::{Path, PathBuf};

use openrows_dump::{PgFormat, ServerVersion, ToolKind};
use openrows_types::QueryId;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt as _;

use super::dto::{
    BackupPickRequest, BackupRequest, DumpContextDto, DumpEngineDto, DumpEventDto, DumpPreviewDto,
    DumpScopeRequest, DumpStarted, DumpToolDto,
};
use super::job::{Job, Work, execute};
use super::plan::{BackupPlan, backup_command, shown};
use super::tools::{Overrides, Resolved, resolve};
use super::{DumpJobs, Scope, blocking, dump_error, endpoint, engine_tools, scope};
use crate::connections::{Connections, IpcError, IpcErrorKind};
use crate::settings::{SettingsFile, current};

/// Resolve `kinds` for `server` with the Settings ▸ Tools paths (blocking probes).
pub(super) async fn resolved(
    app: &AppHandle,
    kinds: &'static [ToolKind],
    server: Option<ServerVersion>,
) -> Result<Vec<Resolved>, IpcError> {
    let overrides = Overrides::from_settings(current(&app.state::<SettingsFile>()).as_ref());
    blocking(move || {
        kinds
            .iter()
            .map(|kind| resolve(*kind, &overrides, server))
            .collect()
    })
    .await
}

/// Every tool as Settings ▸ Tools shows it: the copy found or set there,
/// its version and the other installed copies.
#[tauri::command]
#[specta::specta]
pub(crate) async fn dump_tools(app: AppHandle) -> Result<Vec<DumpToolDto>, IpcError> {
    let tools = resolved(&app, &ToolKind::ALL, None).await?;
    let mut tools: Vec<_> = tools.into_iter().map(|tool| tool.dto).collect();
    tools.extend(super::cloud_tools::list(&app).await);
    Ok(tools)
}

/// What the backup and restore sheets of a session's database need: the
/// engine, the safety switches and the tools with compatibility warnings.
#[tauri::command]
#[specta::specta]
pub(crate) async fn dump_context(
    app: AppHandle,
    request: DumpScopeRequest,
) -> Result<DumpContextDto, IpcError> {
    let scope = scope(&app.state::<Connections>(), &request)?;
    let tools = resolved(&app, engine_tools(scope.engine), scope.server).await?;
    Ok(DumpContextDto {
        engine: scope.engine,
        connection: scope.profile.name.clone(),
        database: scope.database.clone(),
        read_only: scope.read_only(),
        production: scope.production(),
        tools: tools.into_iter().map(|tool| tool.dto).collect(),
    })
}

/// The backup tool of an engine.
fn dump_tool(engine: DumpEngineDto) -> &'static [ToolKind] {
    match engine {
        DumpEngineDto::Mysql => &[ToolKind::MysqlDump],
        DumpEngineDto::Postgres | DumpEngineDto::Sqlite => &[ToolKind::PgDump],
    }
}

/// The backup of `request` with `output`; resolves the tool.
async fn backup_work(
    app: &AppHandle,
    scope: &Scope,
    request: &BackupRequest,
    output: PathBuf,
) -> Result<(Work, Vec<String>), IpcError> {
    if scope.engine == DumpEngineDto::Sqlite {
        let source = scope.sqlite_file()?.to_path_buf();
        return Ok((Work::SqliteBackup { source, output }, Vec::new()));
    }
    let options = request.options.options();
    options.validate().map_err(|error| dump_error(&error))?;
    let mut tools = resolved(app, dump_tool(scope.engine), scope.server).await?;
    let found = tools
        .pop()
        .ok_or_else(|| IpcError::new(IpcErrorKind::Internal, "No backup tool"))?;
    let warnings = found.dto.warning.clone().into_iter().collect();
    let plan = BackupPlan {
        tool: found.require()?,
        mysql: scope.engine == DumpEngineDto::Mysql,
        options,
        output,
        database: scope.database.clone(),
    };
    Ok((Work::Backup(plan), warnings))
}

/// The exact command a backup would run, with the password file shown as
/// `<password file>`; nothing runs and no secret is read.
#[tauri::command]
#[specta::specta]
pub(crate) async fn dump_backup_preview(
    app: AppHandle,
    request: BackupRequest,
) -> Result<DumpPreviewDto, IpcError> {
    let scope = scope(&app.state::<Connections>(), &request.scope)?;
    let output = PathBuf::from(request.output.as_deref().unwrap_or("<file>"));
    let (work, warnings) = backup_work(&app, &scope, &request, output).await?;
    let command = match &work {
        Work::Backup(plan) => {
            let spec = endpoint::preview(&scope.profile)?;
            shown(&backup_command(plan, &spec).map_err(|error| dump_error(&error))?)
        }
        Work::SqliteBackup { output, .. } => format!("VACUUM INTO '{}'", output.display()),
        Work::Restore(_) | Work::SqliteRestore { .. } => String::new(),
    };
    Ok(DumpPreviewDto {
        commands: vec![command],
        warnings,
    })
}

/// The native save panel for a backup of the session's database; `None`
/// when cancelled. Only paths picked here can be written.
#[tauri::command]
#[specta::specta]
pub(crate) async fn dump_backup_pick(
    window: tauri::WebviewWindow,
    app: AppHandle,
    request: BackupPickRequest,
) -> Result<Option<String>, IpcError> {
    let scope_request = DumpScopeRequest {
        session_id: request.session_id,
        server_product: None,
        server_version: None,
    };
    let scope = scope(&app.state::<Connections>(), &scope_request)?;
    let extension = match scope.engine {
        DumpEngineDto::Sqlite => Some("sqlite"),
        DumpEngineDto::Mysql => Some("sql"),
        DumpEngineDto::Postgres => PgFormat::from(request.format).extension(),
    };
    let stem = crate::exports::commands::file_stem(&scope.database);
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let mut panel = window
        .dialog()
        .file()
        .set_parent(&window)
        .set_title("Back Up Database")
        .set_can_create_directories(true);
    panel = match extension {
        Some(extension) => panel
            .set_file_name(format!("{stem}.{extension}"))
            .add_filter(extension.to_uppercase(), &[extension]),
        None => panel.set_file_name(stem),
    };
    panel.save_file(move |picked| {
        // The receiver only disappears when the command was dropped.
        let _ = sender.send(picked);
    });
    let picked = receiver
        .await
        .map_err(|_| IpcError::new(IpcErrorKind::Internal, "The save panel closed unexpectedly"))?;
    let Some(file) = picked else {
        return Ok(None);
    };
    let path = file
        .into_path()
        .map_err(|error| IpcError::invalid(error.to_string()))?;
    app.state::<DumpJobs>().pick(path.clone());
    Ok(Some(path.display().to_string()))
}

/// Make room for a backup at `path`: the save panel confirmed replacing a
/// file; an existing folder is never replaced.
fn clear_output(path: &Path) -> Result<(), IpcError> {
    if path.is_dir() {
        return Err(IpcError::invalid(format!(
            "{} is an existing folder: choose a new name",
            path.display()
        )));
    }
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(IpcError::new(
            IpcErrorKind::Storage,
            format!("{}: {error}", path.display()),
        )),
        _ => Ok(()),
    }
}

/// Start `work` as a background job of `scope`.
pub(super) fn start(
    app: &AppHandle,
    scope: &Scope,
    work: Work,
    events: Channel<DumpEventDto>,
) -> DumpStarted {
    let job = Job {
        id: QueryId::new().to_string(),
        profile_id: scope.profile_id(),
        database: scope.database.clone(),
        work,
        cancel: openrows_dump::Canceller::default(),
        events,
    };
    app.state::<DumpJobs>().track(&job.id, job.cancel.clone());
    let reply = DumpStarted {
        job_id: job.id.clone(),
    };
    tauri::async_runtime::spawn(execute(app.clone(), job));
    reply
}

/// Back up the session's database to the picked file or folder in the
/// background; events follow on `on_event`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn dump_backup_start(
    app: AppHandle,
    request: BackupRequest,
    on_event: Channel<DumpEventDto>,
) -> Result<DumpStarted, IpcError> {
    let scope = scope(&app.state::<Connections>(), &request.scope)?;
    let output = request
        .output
        .as_deref()
        .ok_or_else(|| IpcError::invalid("Choose where to save the backup"))?;
    let output = app.state::<DumpJobs>().picked(output)?;
    let (work, _) = backup_work(&app, &scope, &request, output.clone()).await?;
    clear_output(&output)?;
    Ok(start(&app, &scope, work, on_event))
}

/// Stop a running backup or restore; it reports `cancelled` when it ended.
#[tauri::command]
#[specta::specta]
pub(crate) async fn dump_cancel(jobs: State<'_, DumpJobs>, job_id: String) -> Result<(), IpcError> {
    if jobs.cancel(&job_id) {
        Ok(())
    } else {
        Err(IpcError::new(
            IpcErrorKind::NotFound,
            "The job already ended",
        ))
    }
}
