// SPDX-License-Identifier: Apache-2.0
//! A started export job: reads its plan into the chosen file (or folder) in
//! the background, on a session of its own when it runs statements, so
//! several exports run in parallel and closing a tab does not stop them.
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use openrows_core::export::{
    ExportFailure, ExportJob, ExportOutcome, Progress, export_batches, export_parts,
    export_statement,
};
use openrows_core::script::ScriptCanceller;
use openrows_export::{ExportError, Exporter};
use openrows_results::ResultStore;
use openrows_sql::select::BoundSql;
use openrows_types::SessionId;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};

use super::dto::{ExportEventDto, rows, terminal};
use super::format_dto::ExportFormatDto;
use super::outputs::ExportOutputs;
use super::pins::ResultPins;
use super::plan::Plan;
use super::selected::Selected;
use crate::connections::job_session;
use crate::connections::{Connections, IpcError};
use crate::jobs::JobScope as Scope;
use crate::queries::Queries;

/// Where the rows go.
pub(super) enum Destination {
    /// One file, already created as a temporary file.
    File(PathBuf, Exporter),
    /// A folder receiving one file per table.
    Folder(PathBuf),
}

/// Everything a started export needs after the command returned.
pub(super) struct Job {
    pub(super) export_id: String,
    pub(super) scope: Scope,
    pub(super) plan: Plan,
    pub(super) destination: Destination,
    pub(super) format: ExportFormatDto,
    pub(super) cancel: ScriptCanceller,
    pub(super) events: Channel<ExportEventDto>,
}

/// How a job ended before or after it reached the core.
enum Ended {
    Outcome(ExportOutcome),
    Refused(IpcError),
}

/// Run the job and send its terminal event.
pub(super) async fn execute(app: AppHandle, job: Job) {
    let (path, ended) = match job.destination {
        Destination::File(path, exporter) => {
            let job_parts = (job.plan, exporter);
            let ended = into_file(&app, (&job.scope, &job.cancel, &job.events), job_parts).await;
            (path, ended)
        }
        Destination::Folder(folder) => {
            let ended = into_folder(
                &app,
                (&job.scope, &job.cancel, &job.events),
                (job.plan, &folder, &job.format),
            )
            .await;
            (folder, ended)
        }
    };
    let event = match ended {
        Ended::Outcome(outcome) => {
            if matches!(outcome, ExportOutcome::Written { .. }) {
                app.state::<ExportOutputs>().record(path.clone());
            }
            terminal(outcome, path.display().to_string())
        }
        Ended::Refused(error) => ExportEventDto::Failed { error },
    };
    // A closed channel means the window is gone; the file is complete anyway.
    let _ = job.events.send(event);
    app.state::<Queries>().forget(&job.export_id);
}

type Context<'a> = (&'a Scope, &'a ScriptCanceller, &'a Channel<ExportEventDto>);

fn progress(events: Channel<ExportEventDto>, (before, detail): (u64, Option<String>)) -> Progress {
    Box::new(move |count| {
        // A closed channel means the window is gone; the export still finishes.
        let _ = events.send(ExportEventDto::Progress {
            rows: rows(before + count),
            detail: detail.clone(),
        });
    })
}

async fn into_file(
    app: &AppHandle,
    (scope, cancel, events): Context<'_>,
    (plan, exporter): (Plan, Exporter),
) -> Ended {
    let job = ExportJob {
        exporter,
        cancel: cancel.clone(),
        progress: progress(events.clone(), (0, None)),
    };
    match plan {
        Plan::Stored {
            result_id,
            schema,
            batches,
            rows,
        } => {
            let pins = app.state::<ResultPins>();
            pins.pin(&result_id);
            let outcome = match rows {
                Some(rows) => export_batches((schema, Selected::new(batches, rows)), job).await,
                None => export_batches((schema, batches), job).await,
            };
            if pins.unpin(&result_id) {
                app.state::<Arc<ResultStore>>().remove(&result_id);
            }
            Ended::Outcome(outcome)
        }
        Plan::Statements(parts) => {
            on_own_session(app, scope, |connections, session| async move {
                export_parts(&connections.sessions, session, parts, job).await
            })
            .await
        }
        Plan::PerTable(_) => {
            Ended::Refused(IpcError::invalid("Choose a folder for one file per table"))
        }
    }
}

/// Run `work` on a new session of the scope's database, closed afterwards.
async fn on_own_session<'a, F, W>(app: &'a AppHandle, scope: &Scope, work: W) -> Ended
where
    W: FnOnce(&'a Connections, SessionId) -> F,
    F: Future<Output = ExportOutcome>,
{
    let connections: &'a Connections = app.state::<Connections>().inner();
    let (session, owned) = match job_session(connections, &scope.profile_id, &scope.database).await
    {
        Ok(opened) => opened,
        Err(error) => return Ended::Refused(error),
    };
    let outcome = work(connections, session).await;
    if owned {
        // Best effort: the session is ours and nothing else uses it.
        let _ = connections.sessions.close(session).await;
    }
    Ended::Outcome(outcome)
}

async fn into_folder(
    app: &AppHandle,
    (scope, cancel, events): Context<'_>,
    (plan, folder, format): (Plan, &PathBuf, &ExportFormatDto),
) -> Ended {
    let Plan::PerTable(tables) = plan else {
        return Ended::Refused(IpcError::invalid(
            "Only several tables export into a folder",
        ));
    };
    let context = (scope, cancel, events);
    on_own_session(app, scope, |connections, session| async move {
        let mut tables_run = TablesRun {
            folder,
            format,
            names: HashSet::new(),
            total: 0,
        };
        tables_run.run(connections, session, context, tables).await
    })
    .await
}

/// One file per table, written one after another.
struct TablesRun<'a> {
    folder: &'a PathBuf,
    format: &'a ExportFormatDto,
    /// Lower-case file names used so far.
    names: HashSet<String>,
    total: u64,
}

impl TablesRun<'_> {
    async fn run(
        &mut self,
        connections: &Connections,
        session: SessionId,
        (scope, cancel, events): Context<'_>,
        tables: Vec<(String, BoundSql)>,
    ) -> ExportOutcome {
        let count = tables.len();
        for (index, (table, statement)) in tables.into_iter().enumerate() {
            if cancel.is_cancelled() {
                return ExportOutcome::Cancelled;
            }
            let exporter = match self.exporter(scope, &table) {
                Ok(exporter) => exporter,
                Err(error) => return ExportOutcome::Failed(ExportFailure::Write(error)),
            };
            let detail = format!("{table} ({} of {count})", index + 1);
            let job = ExportJob {
                exporter,
                cancel: cancel.clone(),
                progress: progress(events.clone(), (self.total, Some(detail))),
            };
            match export_statement(&connections.sessions, session, statement, job).await {
                ExportOutcome::Written { rows } => self.total += rows,
                other => return other,
            }
        }
        ExportOutcome::Written { rows: self.total }
    }

    /// The table's file in the folder: its name made safe and unique.
    fn exporter(&mut self, scope: &Scope, table: &str) -> Result<Exporter, ExportError> {
        let format = self.format.for_table(table);
        let (extension, _) = format.extension();
        let stem = super::commands::file_stem(table);
        let mut name = format!("{stem}.{extension}");
        let mut number = 1;
        while !self.names.insert(name.to_lowercase()) {
            number += 1;
            name = format!("{stem} ({number}).{extension}");
        }
        // The options were validated when the export started; only the name can fail.
        let core = format
            .to_format(scope.dialect(), table)
            .map_err(|_| ExportError::InvalidName(table.to_owned()))?;
        Exporter::create(&self.folder.join(name), &core)
    }
}
