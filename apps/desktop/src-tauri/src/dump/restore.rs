// SPDX-License-Identifier: Apache-2.0
//! `dump_restore_pick`, `dump_restore_contents`, `dump_restore_check` and
//! `dump_restore_start`. A restore writes, so it passes the gate: refused
//! on read-only connections, confirmed with a token bound to exactly this
//! restore, and for production connections, `clean` and whole-file
//! `SQLite` restores also by typing the database name.
use std::io::Read as _;
use std::path::{Path, PathBuf};

use openrows_dump::{
    Clean, Confirmation, DumpError, DumpKind, RestoreGate, RestoreRisk, ToolKind, admit, capture,
    confirmation_token, detect, parse_list, pg_list, restore_gate,
};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt as _;

use super::commands::{resolved, start};
use super::dto::{
    ArchiveEntryDto, DumpEngineDto, DumpEventDto, DumpFileKindDto, DumpStarted, PickedDumpDto,
    RestoreCheckDto, RestoreContentsRequest, RestoreRequest, RestoreTargetDto,
};
use super::job::Work;
use super::plan::{RestorePlan, restore_preview, shown};
use super::{DumpJobs, Scope, blocking, dump_error, endpoint, scope};
use crate::connections::{Connections, IpcError, IpcErrorKind};

/// The first bytes of every `SQLite` 3 database file.
const SQLITE_MAGIC: &[u8] = b"SQLite format 3\0";

/// What a picked file or folder holds.
fn file_kind(path: &Path) -> Result<DumpFileKindDto, DumpError> {
    if path.is_file() {
        let mut head = [0; 16];
        let read = std::fs::File::open(path)
            .and_then(|mut file| file.read(&mut head))
            .map_err(|error| DumpError::Io {
                context: format!("cannot read {}", path.display()),
                source: error,
            })?;
        if head[..read] == *SQLITE_MAGIC {
            return Ok(DumpFileKindDto::Sqlite);
        }
    }
    detect(path).map(DumpFileKindDto::from)
}

/// The native open panel for a backup file (or a `pg_dump` folder); `None`
/// when cancelled. Only paths picked here can be restored.
#[tauri::command]
#[specta::specta]
pub(crate) async fn dump_restore_pick(
    window: tauri::WebviewWindow,
    app: AppHandle,
    folder: bool,
) -> Result<Option<PickedDumpDto>, IpcError> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let panel = window.dialog().file().set_parent(&window);
    let reply = move |picked| {
        // The receiver only disappears when the command was dropped.
        let _ = sender.send(picked);
    };
    if folder {
        panel.set_title("Restore from Folder").pick_folder(reply);
    } else {
        panel
            .set_title("Restore Database")
            .add_filter(
                "Backups",
                &["sql", "dump", "backup", "tar", "sqlite", "db", "sqlite3"],
            )
            .add_filter("All files", &["*"])
            .pick_file(reply);
    }
    let picked = receiver
        .await
        .map_err(|_| IpcError::new(IpcErrorKind::Internal, "The open panel closed unexpectedly"))?;
    let Some(file) = picked else {
        return Ok(None);
    };
    let path = file
        .into_path()
        .map_err(|error| IpcError::invalid(error.to_string()))?;
    let checked = path.clone();
    let kind = blocking(move || file_kind(&checked))
        .await?
        .map_err(|error| dump_error(&error))?;
    app.state::<DumpJobs>().pick(path.clone());
    let bytes = std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
    #[expect(clippy::cast_precision_loss, reason = "file sizes stay far below 2^53")]
    let bytes = bytes as f64;
    Ok(Some(PickedDumpDto {
        name: path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default(),
        path: path.display().to_string(),
        kind,
        bytes,
    }))
}

/// The table of contents of a picked `pg_dump` archive (`pg_restore --list`).
#[tauri::command]
#[specta::specta]
pub(crate) async fn dump_restore_contents(
    app: AppHandle,
    request: RestoreContentsRequest,
) -> Result<Vec<ArchiveEntryDto>, IpcError> {
    let scope = scope(&app.state::<Connections>(), &request.scope)?;
    let path = app.state::<DumpJobs>().picked(&request.path)?;
    let mut tools = resolved(&app, &[ToolKind::PgRestore], scope.server).await?;
    let tool = tools
        .pop()
        .ok_or_else(|| IpcError::new(IpcErrorKind::Internal, "No pg_restore"))?
        .require()?;
    let listed = blocking(move || capture(&pg_list(&tool, &path)))
        .await?
        .map_err(|error| dump_error(&error))?;
    Ok(parse_list(&listed)
        .into_iter()
        .map(|entry| ArchiveEntryDto {
            id: entry.id,
            label: entry.label,
            data: entry.data,
        })
        .collect())
}

/// A restore, checked and ready to confirm or start.
struct Prepared {
    scope: Scope,
    work: Work,
    database: String,
    gate: RestoreGate,
    token: String,
    warnings: Vec<String>,
}

/// The database a restore writes to, and whether it is created first.
fn target(scope: &Scope, target: &RestoreTargetDto) -> Result<(String, bool), IpcError> {
    match target {
        RestoreTargetDto::Existing => Ok((scope.database.clone(), false)),
        RestoreTargetDto::Create { .. } if scope.engine == DumpEngineDto::Sqlite => Err(
            IpcError::invalid("A SQLite backup is restored over the open database file"),
        ),
        RestoreTargetDto::Create { name } => {
            let name = name.trim();
            if name.is_empty() {
                return Err(IpcError::invalid("Name the new database"));
            }
            Ok((name.to_owned(), true))
        }
    }
}

/// Whether `kind` restores on `engine`.
fn check_kind(engine: DumpEngineDto, kind: DumpFileKindDto) -> Result<(), IpcError> {
    let fits = match engine {
        DumpEngineDto::Sqlite => kind == DumpFileKindDto::Sqlite,
        DumpEngineDto::Mysql => kind == DumpFileKindDto::PlainSql,
        DumpEngineDto::Postgres => kind != DumpFileKindDto::Sqlite,
    };
    if fits {
        return Ok(());
    }
    let what = match kind {
        DumpFileKindDto::Sqlite => "a SQLite database",
        DumpFileKindDto::PlainSql => "a SQL script",
        _ => "a PostgreSQL archive",
    };
    Err(IpcError::invalid(format!(
        "This file is {what}, which cannot be restored on this connection"
    )))
}

fn dump_kind(kind: DumpFileKindDto) -> DumpKind {
    match kind {
        DumpFileKindDto::PgCustom => DumpKind::PgCustom,
        DumpFileKindDto::PgTar => DumpKind::PgTar,
        DumpFileKindDto::PgDirectory => DumpKind::PgDirectory,
        DumpFileKindDto::PlainSql | DumpFileKindDto::Sqlite => DumpKind::PlainSql,
    }
}

async fn prepare(app: &AppHandle, request: &RestoreRequest) -> Result<Prepared, IpcError> {
    let scope = scope(&app.state::<Connections>(), &request.scope)?;
    let path = app.state::<DumpJobs>().picked(&request.path)?;
    let checked = path.clone();
    let kind = blocking(move || file_kind(&checked))
        .await?
        .map_err(|error| dump_error(&error))?;
    check_kind(scope.engine, kind)?;
    let (database, create) = target(&scope, &request.target)?;
    let options = request.options.options();
    options
        .validate(kind == DumpFileKindDto::PgTar)
        .map_err(|error| dump_error(&error))?;
    let destructive = kind == DumpFileKindDto::Sqlite
        || (kind != DumpFileKindDto::PlainSql && options.clean != Clean::Keep);
    let gate = restore_gate(RestoreRisk {
        read_only: scope.read_only(),
        production: scope.production(),
        destructive,
    });
    let token = confirmation_token(&(scope.profile_id(), &database, create, &path, kind, &options));
    let (work, warnings) = restore_work(
        app,
        &scope,
        (path, kind),
        (database.clone(), create),
        options,
    )
    .await?;
    Ok(Prepared {
        scope,
        work,
        database,
        gate,
        token,
        warnings,
    })
}

async fn restore_work(
    app: &AppHandle,
    scope: &Scope,
    (path, kind): (PathBuf, DumpFileKindDto),
    (database, create): (String, bool),
    options: openrows_dump::RestoreOptions,
) -> Result<(Work, Vec<String>), IpcError> {
    if kind == DumpFileKindDto::Sqlite {
        let target = scope.sqlite_file()?.to_path_buf();
        return Ok((
            Work::SqliteRestore {
                backup: path,
                target,
            },
            Vec::new(),
        ));
    }
    let mysql = scope.engine == DumpEngineDto::Mysql;
    let kinds: &'static [ToolKind] = if mysql {
        &[ToolKind::Mysql]
    } else {
        &[ToolKind::PgRestore, ToolKind::Psql]
    };
    let tools = resolved(app, kinds, scope.server).await?;
    let warnings = tools
        .iter()
        .filter_map(|tool| tool.dto.warning.clone())
        .collect();
    let mut found = tools.into_iter().map(|tool| tool.tool);
    let (archive_tool, client) = if mysql {
        (None, found.next().flatten())
    } else {
        (found.next().flatten(), found.next().flatten())
    };
    let plan = RestorePlan {
        kind: dump_kind(kind),
        path,
        database,
        create,
        options,
        mysql,
        archive_tool,
        client,
    };
    Ok((Work::Restore(plan), warnings))
}

/// Check a restore without running it: the exact commands, warnings, and
/// how it must be confirmed (or why it is refused).
#[tauri::command]
#[specta::specta]
pub(crate) async fn dump_restore_check(
    app: AppHandle,
    request: RestoreRequest,
) -> Result<RestoreCheckDto, IpcError> {
    let prepared = prepare(&app, &request).await?;
    let commands = match &prepared.work {
        Work::Restore(plan) => {
            let spec = endpoint::preview(&prepared.scope.profile)?;
            let commands = restore_preview(plan, &spec).map_err(|error| dump_error(&error))?;
            commands.iter().map(shown).collect()
        }
        Work::SqliteRestore { backup, target } => vec![format!(
            "Copy {} over {} (SQLite backup API)",
            backup.display(),
            target.display()
        )],
        Work::Backup(_) | Work::SqliteBackup { .. } => Vec::new(),
    };
    let blocked = prepared.gate == RestoreGate::Blocked;
    Ok(RestoreCheckDto {
        commands,
        warnings: prepared.warnings,
        blocked: blocked.then(|| {
            "This connection is read-only: restoring would modify the database".to_owned()
        }),
        database: prepared.database,
        typed: prepared.gate == RestoreGate::Confirm { typed: true },
        confirm_token: (!blocked).then_some(prepared.token),
    })
}

/// Restore a picked backup in the background after the user confirmed
/// the check; events follow on `on_event`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn dump_restore_start(
    app: AppHandle,
    request: RestoreRequest,
    on_event: Channel<DumpEventDto>,
) -> Result<DumpStarted, IpcError> {
    let prepared = prepare(&app, &request).await?;
    let confirmation = Confirmation {
        token: request.confirm_token.as_deref(),
        typed: request.typed_name.as_deref(),
    };
    admit(
        prepared.gate,
        &prepared.token,
        &prepared.database,
        confirmation,
    )
    .map_err(|error| dump_error(&error))?;
    crate::security::authenticate_plan(
        &restore_safety(&prepared.scope.profile),
        &prepared.database,
    )
    .await?;
    app.state::<crate::security::Security>()
        .admit("dump_restore_start")?;
    Ok(start(&app, &prepared.scope, prepared.work, on_event))
}

fn restore_safety(
    profile: &openrows_store::profile::ConnectionProfile,
) -> openrows_core::safety::SafetyPlan {
    openrows_core::safety::SafetyPlan::check(
        crate::queries::commands::policy(profile),
        vec!["Restore backup using the reviewed native commands".into()],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_restore_passes_the_central_policy_as_an_opaque_write() {
        use openrows_core::safety::Gate;
        use openrows_store::profile::{ConnectionProfile, EnvKind, EnvTag, Safety, ServerParams};
        let mut profile = ConnectionProfile::new_postgres(
            "test",
            ServerParams::default(),
            EnvTag::new(EnvKind::Local),
        );
        for mode in [
            Safety::Unrestricted,
            Safety::ConfirmAll,
            Safety::ConfirmWrites,
            Safety::AuthenticateAll,
            Safety::AuthenticateWrites,
            Safety::ReadOnly,
        ] {
            profile.safety = mode;
            let plan = restore_safety(&profile);
            assert_eq!(
                plan.gate(),
                match mode {
                    Safety::Unrestricted => Gate::Run,
                    Safety::ReadOnly => Gate::Block,
                    _ => Gate::Confirm,
                }
            );
            assert_eq!(plan.authentication_required, mode.authenticates());
        }
    }

    #[test]
    fn sqlite_files_are_told_apart_from_dumps() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let database = dir.path().join("a.db");
        std::fs::write(&database, b"SQLite format 3\0rest of the header")?;
        assert_eq!(file_kind(&database)?, DumpFileKindDto::Sqlite);
        let script = dir.path().join("a.sql");
        std::fs::write(&script, "SELECT 1;")?;
        assert_eq!(file_kind(&script)?, DumpFileKindDto::PlainSql);
        Ok(())
    }

    #[test]
    fn files_restore_only_on_their_engine() {
        assert!(check_kind(DumpEngineDto::Postgres, DumpFileKindDto::PgCustom).is_ok());
        assert!(check_kind(DumpEngineDto::Mysql, DumpFileKindDto::PlainSql).is_ok());
        assert!(check_kind(DumpEngineDto::Sqlite, DumpFileKindDto::Sqlite).is_ok());
        let wrong = check_kind(DumpEngineDto::Mysql, DumpFileKindDto::PgTar);
        assert!(wrong.is_err_and(|error| error.message.contains("PostgreSQL archive")));
        assert!(check_kind(DumpEngineDto::Postgres, DumpFileKindDto::Sqlite).is_err());
    }
}
