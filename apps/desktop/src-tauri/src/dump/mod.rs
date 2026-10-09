// SPDX-License-Identifier: Apache-2.0
//! Backup and restore with the user's installed client tools (M2-14,
//! FR-IO-04), and a native copy for `SQLite`.
//!
//! [`commands::dump_context`] reports the engine and the copy of each tool
//! a job would use ([`tools`]: Settings ▸ Tools paths, else the installed
//! copy best matching the server, with a warning when it is too old);
//! [`commands::dump_backup_preview`] and [`restore::dump_restore_check`]
//! show the exact command lines before anything runs. Only files picked in
//! this run's panels are read or written. Jobs ([`job`]) log in through
//! [`endpoint`]: the password goes to a 0600 file, never the command line,
//! and routed profiles get a loopback [`forward`] to their SSH tunnel or
//! proxy. Restores pass the gate of `openrows_dump::admit` (read-only
//! connections never restore; production and destructive restores need
//! the database name typed). `dump_cancel` signals the tool's process group.
pub(crate) mod commands;
mod dto;
mod endpoint;
mod forward;
mod job;
mod plan;
pub(crate) mod restore;
mod tools;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::str::FromStr as _;
use std::sync::{Mutex, MutexGuard, PoisonError};

use openrows_dump::{Canceller, ServerVersion, ToolKind};
use openrows_store::profile::{ConnectionProfile, Driver, EnvKind};
use openrows_types::SessionId;

use crate::connections::{Connections, IpcError, IpcErrorKind};
use dto::{DumpEngineDto, DumpScopeRequest};

/// Files picked in this run's panels and the cancel handles of running jobs.
#[derive(Default)]
pub(crate) struct DumpJobs {
    picked: Mutex<HashSet<PathBuf>>,
    running: Mutex<HashMap<String, Canceller>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl DumpJobs {
    fn pick(&self, path: PathBuf) {
        lock(&self.picked).insert(path);
    }

    /// `path` when it was picked in a panel of this run.
    fn picked(&self, path: &str) -> Result<PathBuf, IpcError> {
        let path = PathBuf::from(path);
        if lock(&self.picked).contains(&path) {
            Ok(path)
        } else {
            Err(IpcError::invalid("Choose the file with the panel first"))
        }
    }

    fn track(&self, id: &str, cancel: Canceller) {
        lock(&self.running).insert(id.to_owned(), cancel);
    }

    fn forget(&self, id: &str) {
        lock(&self.running).remove(id);
    }

    fn cancel(&self, id: &str) -> bool {
        lock(&self.running).get(id).map(Canceller::cancel).is_some()
    }
}

/// The saved connection, database and server a dump command acts on.
pub(crate) struct Scope {
    profile: ConnectionProfile,
    database: String,
    engine: DumpEngineDto,
    server: Option<ServerVersion>,
}

impl Scope {
    fn profile_id(&self) -> String {
        self.profile.id.to_string()
    }

    /// Restores are refused: read-only connection or file.
    fn read_only(&self) -> bool {
        let file = self
            .profile
            .file
            .as_ref()
            .is_some_and(|file| file.read_only);
        self.profile.safety.read_only || file
    }

    fn production(&self) -> bool {
        self.profile.env_tag.kind == EnvKind::Prod
    }

    /// The `SQLite` database file.
    fn sqlite_file(&self) -> Result<&Path, IpcError> {
        match &self.profile.file {
            Some(file) if !file.memory => Ok(&file.path),
            _ => Err(IpcError::new(
                IpcErrorKind::Unsupported,
                "In-memory databases cannot be backed up or restored",
            )),
        }
    }
}

/// The scope of an open webview session.
fn scope(connections: &Connections, request: &DumpScopeRequest) -> Result<Scope, IpcError> {
    let session = SessionId::from_str(&request.session_id)
        .map_err(|_| IpcError::invalid("Invalid session id"))?;
    let owner = connections
        .owners
        .owner(session)
        .ok_or_else(|| IpcError::new(IpcErrorKind::NotFound, "The session is not open"))?;
    let profile = connections.profiles.get(owner.profile)?;
    let engine = match profile.driver {
        Driver::Postgres => DumpEngineDto::Postgres,
        Driver::Mysql | Driver::Mariadb => DumpEngineDto::Mysql,
        Driver::Sqlite => DumpEngineDto::Sqlite,
        Driver::Sqlserver => {
            return Err(IpcError::new(
                IpcErrorKind::Unsupported,
                "Backup and restore are not available for SQL Server yet",
            ));
        }
    };
    Ok(Scope {
        database: owner.database,
        server: request.server(engine),
        engine,
        profile,
    })
}

/// The tools an engine's jobs use, in Settings order.
fn engine_tools(engine: DumpEngineDto) -> &'static [ToolKind] {
    match engine {
        DumpEngineDto::Postgres => &[ToolKind::PgDump, ToolKind::PgRestore, ToolKind::Psql],
        DumpEngineDto::Mysql => &[ToolKind::MysqlDump, ToolKind::Mysql],
        DumpEngineDto::Sqlite => &[],
    }
}

/// Run blocking work (probing tools, reading files) off the async runtime.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, IpcError> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| IpcError::new(IpcErrorKind::Internal, error.to_string()))
}

/// A crate error as an IPC error, keeping its message.
fn dump_error(error: &openrows_dump::DumpError) -> IpcError {
    let kind = match error {
        openrows_dump::DumpError::NotFound { .. } => IpcErrorKind::NotFound,
        openrows_dump::DumpError::Io { .. } => IpcErrorKind::Storage,
        _ => IpcErrorKind::Invalid,
    };
    IpcError::new(kind, crate::exports::capitalised(&error.to_string()))
}
