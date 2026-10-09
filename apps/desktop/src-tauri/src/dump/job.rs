// SPDX-License-Identifier: Apache-2.0
//! Run a backup or restore in the background: log in, run each step's tool
//! on a blocking thread with log, progress and cancel, clean up a partial
//! backup, and report how it ended.
use std::path::PathBuf;
use std::time::Instant;

use openrows_dump::{
    Canceller, DumpError, RunEvent, RunOutcome, run, sqlite_backup, sqlite_restore,
};
use openrows_sql::StatementKind;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};

use super::dto::{DumpEventDto, DumpOutcomeDto};
use super::endpoint::login;
use super::plan::{BackupPlan, Built, RestorePlan, backup_command, restore_built};
use super::{DumpJobs, blocking};
use crate::connections::catalog_cache::after_statements;
use crate::connections::{Connections, IpcError};
use crate::exports::outputs::ExportOutputs;

/// What a job does.
pub(super) enum Work {
    Backup(BackupPlan),
    Restore(RestorePlan),
    SqliteBackup { source: PathBuf, output: PathBuf },
    SqliteRestore { backup: PathBuf, target: PathBuf },
}

impl Work {
    /// The backup's file or folder.
    fn output(&self) -> Option<&PathBuf> {
        match self {
            Self::Backup(plan) => Some(&plan.output),
            Self::SqliteBackup { output, .. } => Some(output),
            Self::Restore(_) | Self::SqliteRestore { .. } => None,
        }
    }

    /// The database a restore wrote to.
    fn restored(&self, scope_database: &str) -> Option<String> {
        match self {
            Self::Restore(plan) => Some(plan.database.clone()),
            Self::SqliteRestore { .. } => Some(scope_database.to_owned()),
            Self::Backup(_) | Self::SqliteBackup { .. } => None,
        }
    }
}

/// A started job.
pub(super) struct Job {
    pub(super) id: String,
    pub(super) profile_id: String,
    pub(super) database: String,
    pub(super) work: Work,
    pub(super) cancel: Canceller,
    pub(super) events: Channel<DumpEventDto>,
}

/// Run `job` to its end and report it; never fails (failures are events).
pub(super) async fn execute(app: AppHandle, job: Job) {
    let started = Instant::now();
    let result = match &job.work {
        Work::Backup(_) | Work::Restore(_) => run_tools(&app, &job).await,
        Work::SqliteBackup { .. } | Work::SqliteRestore { .. } => run_sqlite(&job).await,
    };
    let (outcome, exit_code, message) = match result {
        Ok(RunOutcome::Completed) => (DumpOutcomeDto::Completed, None, None),
        Ok(RunOutcome::Cancelled) => (DumpOutcomeDto::Cancelled, None, None),
        Ok(RunOutcome::Failed { code, message }) => (DumpOutcomeDto::Failed, code, Some(message)),
        Err(error) => (DumpOutcomeDto::Failed, None, Some(error.message)),
    };
    let output = job.work.output().cloned();
    if let Some(path) = &output {
        settle_output(&app, path, outcome);
    }
    if let Some(database) = job.work.restored(&job.database) {
        // A restore changes the schema; the tree reads afresh.
        after_statements(&app, (&job.profile_id, &database), [StatementKind::Ddl]);
    }
    let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    #[expect(
        clippy::cast_precision_loss,
        reason = "durations stay far below 2^53 ms"
    )]
    let elapsed_ms = elapsed_ms as f64;
    // A closed channel means the window is gone; the job still ended.
    let _ = job.events.send(DumpEventDto::Finished {
        outcome,
        exit_code,
        message,
        output: output
            .filter(|_| outcome == DumpOutcomeDto::Completed)
            .map(|path| path.display().to_string()),
        elapsed_ms,
    });
    app.state::<DumpJobs>().forget(&job.id);
}

/// A finished backup can be opened from the jobs list; a partial one is removed.
fn settle_output(app: &AppHandle, path: &PathBuf, outcome: DumpOutcomeDto) {
    if outcome == DumpOutcomeDto::Completed {
        app.state::<ExportOutputs>().record(path.clone());
        return;
    }
    // The path did not exist before the job (checked at start), so this
    // only removes what the tool wrote.
    let _ = if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    };
}

async fn run_tools(app: &AppHandle, job: &Job) -> Result<RunOutcome, IpcError> {
    let (mysql, connections) = match &job.work {
        Work::Backup(plan) => (plan.mysql, 2),
        Work::Restore(plan) => (plan.mysql, 2 * plan.connections()),
        _ => return Err(IpcError::invalid("Not a tool job")),
    };
    let login = login(
        &app.state::<Connections>(),
        &job.profile_id,
        mysql,
        connections,
    )
    .await?;
    let built = match &job.work {
        Work::Backup(plan) => backup_command(plan, &login.spec).map(|command| Built {
            commands: vec![command],
            total: None,
            total_bytes: None,
            _files: Vec::new(),
        }),
        Work::Restore(plan) => {
            let (plan, spec) = (plan.clone(), login.spec.clone());
            blocking(move || restore_built(&plan, &spec)).await?
        }
        _ => return Err(IpcError::invalid("Not a tool job")),
    }
    .map_err(|error| super::dump_error(&error))?;
    let (cancel, events) = (job.cancel.clone(), job.events.clone());
    blocking(move || {
        let outcome = run_steps(&built, &cancel, &events);
        // The password file, CA file and forward live until the tools ended.
        drop(login);
        outcome
    })
    .await?
    .map_err(|error| super::dump_error(&error))
}

fn run_steps(
    built: &Built,
    cancel: &Canceller,
    events: &Channel<DumpEventDto>,
) -> Result<RunOutcome, DumpError> {
    #[expect(
        clippy::cast_precision_loss,
        reason = "counts and sizes stay far below 2^53"
    )]
    let number = |value: u64| value as f64;
    for (index, command) in built.commands.iter().enumerate() {
        let _ = events.send(DumpEventDto::Step {
            index: u32::try_from(index).unwrap_or(u32::MAX),
            command: command.display(),
        });
        let outcome = run(command, cancel, |event| {
            let _ = events.send(match event {
                RunEvent::Log(lines) => DumpEventDto::Log { lines },
                RunEvent::Progress { objects, bytes } => DumpEventDto::Progress {
                    objects: number(objects),
                    total: built.total.map(number),
                    bytes: number(bytes),
                    total_bytes: built.total_bytes.map(number),
                },
            });
        })?;
        if outcome != RunOutcome::Completed {
            return Ok(outcome);
        }
    }
    Ok(RunOutcome::Completed)
}

async fn run_sqlite(job: &Job) -> Result<RunOutcome, IpcError> {
    let (command, work): (String, Box<dyn FnOnce() -> Result<u64, DumpError> + Send>) =
        match &job.work {
            Work::SqliteBackup { source, output } => {
                let (source, output) = (source.clone(), output.clone());
                let text = format!("VACUUM INTO '{}'", output.display());
                (text, Box::new(move || sqlite_backup(&source, &output)))
            }
            Work::SqliteRestore { backup, target } => {
                let (backup, target) = (backup.clone(), target.clone());
                let text = format!(
                    "Copy {} over {} (SQLite backup API)",
                    backup.display(),
                    target.display()
                );
                (
                    text,
                    Box::new(move || sqlite_restore(&backup, &target).map(|()| 0)),
                )
            }
            _ => return Err(IpcError::invalid("Not a SQLite job")),
        };
    let _ = job.events.send(DumpEventDto::Step { index: 0, command });
    let bytes = blocking(work)
        .await?
        .map_err(|error| super::dump_error(&error))?;
    #[expect(clippy::cast_precision_loss, reason = "file sizes stay far below 2^53")]
    let bytes = bytes as f64;
    let _ = job.events.send(DumpEventDto::Progress {
        objects: 0.0,
        total: None,
        bytes,
        total_bytes: None,
    });
    Ok(RunOutcome::Completed)
}
