// SPDX-License-Identifier: Apache-2.0
//! `run_file_pick`, `run_file_check` and `run_file_start`.
use std::path::{Path, PathBuf};
use std::time::Instant;

use openrows_core::safety::Gate;
use openrows_core::script::ScriptCanceller;
use openrows_core::script_file::{FileRun, FileScan, run_file, scan_file, supports_transaction};
use openrows_sql::StatementKind;
use openrows_types::QueryId;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt as _;

use super::dto::{
    PickedScriptDto, RunFileCheckDto, RunFileCheckRequest, RunFileEventDto, RunFileStartRequest,
    RunFileStarted, finished,
};
use super::{Check, RunFiles};
use crate::connections::catalog_cache::after_statements;
use crate::connections::sessions::open_session;
use crate::connections::{Connections, IpcError, IpcErrorKind};
use crate::jobs::{JobScope, job_scope};
use crate::queries::Queries;

/// Choose a `.sql` file in the native open panel; `None` when cancelled.
#[tauri::command]
#[specta::specta]
pub(crate) async fn run_file_pick(
    window: tauri::WebviewWindow,
    files: State<'_, RunFiles>,
) -> Result<Option<PickedScriptDto>, IpcError> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    window
        .dialog()
        .file()
        .set_parent(&window)
        .set_title("Run SQL File")
        .add_filter("SQL", &["sql"])
        .add_filter("All files", &["*"])
        .pick_file(move |picked| {
            // The receiver only disappears when the command was dropped.
            let _ = sender.send(picked);
        });
    let picked = receiver
        .await
        .map_err(|_| IpcError::new(IpcErrorKind::Internal, "The open panel closed unexpectedly"))?;
    let Some(file) = picked else {
        return Ok(None);
    };
    let path = file
        .into_path()
        .map_err(|error| IpcError::invalid(error.to_string()))?;
    let bytes = std::fs::metadata(&path)
        .map_err(|error| IpcError::new(IpcErrorKind::Storage, error.to_string()))?
        .len();
    files.pick(path.clone());
    #[expect(clippy::cast_precision_loss, reason = "file sizes stay far below 2^53")]
    let bytes = bytes as f64;
    Ok(Some(PickedScriptDto {
        name: path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default(),
        path: path.display().to_string(),
        bytes,
    }))
}

/// A path the user picked in this run's open panel.
fn picked(files: &RunFiles, path: &str) -> Result<PathBuf, IpcError> {
    let path = PathBuf::from(path);
    if files.is_picked(&path) {
        Ok(path)
    } else {
        Err(IpcError::invalid("Choose the file with Run SQL File…"))
    }
}

/// Read the whole file once and judge every statement under the
/// connection's safety switches; nothing runs.
#[tauri::command]
#[specta::specta]
pub(crate) async fn run_file_check(
    connections: State<'_, Connections>,
    files: State<'_, RunFiles>,
    request: RunFileCheckRequest,
) -> Result<RunFileCheckDto, IpcError> {
    let path = picked(&files, &request.path)?;
    let scope = job_scope(&connections, &request.session_id)?;
    let policy = scope.policy;
    let scanned = path.clone();
    let scan = tauri::async_runtime::spawn_blocking(move || scan_file(&scanned, policy, || false))
        .await
        .map_err(|error| IpcError::new(IpcErrorKind::Internal, error.to_string()))?
        .map_err(|error| IpcError::new(IpcErrorKind::Storage, capitalised(&error.to_string())))?;
    let token = scan.confirmation_token(&scope.key(), &path);
    let reply = RunFileCheckDto::new(&scan, token, supports_transaction(scope.dialect()));
    files.checked(
        path,
        Check {
            scope: scope.key(),
            scan,
        },
    );
    Ok(reply)
}

/// The checked scan of `path` when it may start, else why not.
fn admitted(
    files: &RunFiles,
    (path, scope): (&Path, &JobScope),
    request: &RunFileStartRequest,
) -> Result<FileScan, IpcError> {
    let check = files
        .check(path)
        .filter(|check| check.scope == scope.key())
        .ok_or_else(|| IpcError::invalid("Check the file before running it"))?;
    let scan = check.scan;
    match scan.gate() {
        Gate::Block => Err(IpcError::invalid(
            "This connection is read-only: nothing was run because a statement may modify the database",
        )),
        Gate::Confirm
            if request.confirm_token.as_deref()
                != Some(scan.confirmation_token(&check.scope, path).as_str()) =>
        {
            Err(IpcError::invalid(
                "Confirm running the file's statements first",
            ))
        }
        Gate::Confirm | Gate::Run => {
            if request.transaction && !supports_transaction(scope.dialect()) {
                return Err(IpcError::invalid(
                    "This database commits DDL implicitly, so a file cannot run in one transaction",
                ));
            }
            Ok(scan)
        }
    }
}

/// Everything a started run needs after the command returned.
struct Run {
    id: String,
    scope: JobScope,
    path: PathBuf,
    scan: FileScan,
    stop_on_error: bool,
    transaction: bool,
    cancel: ScriptCanceller,
    events: Channel<RunFileEventDto>,
}

/// Run a checked file on a session of its own, in the background.
#[tauri::command]
#[specta::specta]
pub(crate) async fn run_file_start(
    app: AppHandle,
    request: RunFileStartRequest,
    on_event: Channel<RunFileEventDto>,
) -> Result<RunFileStarted, IpcError> {
    let files = app.state::<RunFiles>();
    let path = picked(&files, &request.path)?;
    let scope = job_scope(&app.state::<Connections>(), &request.session_id)?;
    let scan = admitted(&files, (&path, &scope), &request)?;
    let profile = app
        .state::<Connections>()
        .profiles
        .get(crate::connections::profiles::profile_id(&scope.profile_id)?)?;
    if scope.policy.mode.authenticates()
        && (scope.policy.mode == openrows_types::safety::Safety::AuthenticateAll
            || scan.confirm > 0)
    {
        crate::security::authorize(
            &profile,
            vec!["Run the reviewed SQL file statements".into()],
            &scope.database,
        )
        .await?;
    }
    app.state::<crate::security::Security>()
        .admit("run_file_start")?;
    let run = Run {
        id: QueryId::new().to_string(),
        scope,
        path,
        scan,
        stop_on_error: request.stop_on_error,
        transaction: request.transaction,
        cancel: ScriptCanceller::default(),
        events: on_event,
    };
    app.state::<Queries>().track(&run.id, run.cancel.clone());
    let reply = RunFileStarted {
        run_id: run.id.clone(),
    };
    tauri::async_runtime::spawn(execute(app.clone(), run));
    Ok(reply)
}

async fn execute(app: AppHandle, run: Run) {
    let started = Instant::now();
    let connections = app.state::<Connections>();
    let opened = open_session(
        &connections,
        &run.scope.profile_id,
        Some(&run.scope.database),
    )
    .await;
    let summary = match opened {
        Ok((session, _)) => {
            let file = FileRun {
                path: &run.path,
                policy: run.scope.policy,
                stop_on_error: run.stop_on_error,
                transaction: run.transaction,
                stamp: Some(run.scan.stamp),
            };
            let summary = run_file(&connections.sessions, session, file, &run.cancel, |event| {
                // A closed channel means the window is gone; the run still finishes.
                let _ = run.events.send(event.into());
            })
            .await;
            // Best effort: the session is ours and nothing else uses it.
            let _ = connections.sessions.close(session).await;
            Some(summary)
        }
        Err(error) => {
            let _ = run.events.send(failed_to_open(error.message));
            None
        }
    };
    if let Some(summary) = summary {
        // A script may run DDL; the schema tree reads afresh.
        let scope = (run.scope.profile_id.as_str(), run.scope.database.as_str());
        after_statements(&app, scope, [StatementKind::Ddl]);
        let elapsed = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let _ = run.events.send(finished(summary, elapsed));
    }
    app.state::<Queries>().forget(&run.id);
}

fn failed_to_open(message: String) -> RunFileEventDto {
    RunFileEventDto::Finished {
        outcome: super::dto::RunFileOutcomeDto::Failed,
        statements: 0.0,
        errors: 0.0,
        rows_affected: 0.0,
        rolled_back: false,
        message: Some(message),
        elapsed_ms: 0.0,
    }
}

fn capitalised(message: &str) -> String {
    crate::exports::capitalised(message)
}
