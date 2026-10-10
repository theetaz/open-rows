// SPDX-License-Identifier: Apache-2.0
//! The steps of a backup or restore as tool commands for a login: built
//! for display (previews, with stand-ins for what only exists while a job
//! runs) or for running (with the archive's use-list written to a 0600
//! file).
use std::path::{Path, PathBuf};

use openrows_dump::{
    BackupOptions, ConnectionSpec, DumpError, DumpKind, FoundTool, PrivateFile, RestoreOptions,
    ToolCommand, capture, mysql_create_database, mysql_script, mysqldump, parse_list, pg_dump,
    pg_list, pg_restore, psql_create_database, psql_script, use_list,
};

use super::endpoint::{PREVIEW_PORT, TUNNEL_PORT_MARK};

/// Stands in for the use-list file in previews.
const PREVIEW_LIST: &str = "<selected items>";

/// A backup with a tool.
#[derive(Clone)]
pub(super) struct BackupPlan {
    pub(super) tool: FoundTool,
    pub(super) mysql: bool,
    pub(super) options: BackupOptions,
    pub(super) output: PathBuf,
    pub(super) database: String,
}

/// A restore with tools.
#[derive(Clone)]
pub(super) struct RestorePlan {
    pub(super) kind: DumpKind,
    pub(super) path: PathBuf,
    /// The database written to.
    pub(super) database: String,
    /// Create `database` first.
    pub(super) create: bool,
    pub(super) options: RestoreOptions,
    pub(super) mysql: bool,
    /// `pg_restore`, for archives.
    pub(super) archive_tool: Option<FoundTool>,
    /// `psql` or `mysql`, for scripts and creating the database.
    pub(super) client: Option<FoundTool>,
}

impl RestorePlan {
    /// Connections the tools open (a `-j` restore opens one per job).
    pub(super) fn connections(&self) -> usize {
        usize::from(self.create) + usize::from(self.options.jobs.max(1)) + 1
    }
}

/// What a job runs, built for a live login.
pub(super) struct Built {
    pub(super) commands: Vec<ToolCommand>,
    /// Objects the restore will report, when known.
    pub(super) total: Option<u64>,
    /// Bytes a script restore pipes in.
    pub(super) total_bytes: Option<u64>,
    /// Files the commands read; deleted when dropped.
    pub(super) _files: Vec<PrivateFile>,
}

pub(super) fn backup_command(
    plan: &BackupPlan,
    spec: &ConnectionSpec,
) -> Result<ToolCommand, DumpError> {
    let spec = spec.with_database(&plan.database);
    if plan.mysql {
        mysqldump(&plan.tool, &spec, &plan.options, &plan.output)
    } else {
        pg_dump(&plan.tool, &spec, &plan.options, &plan.output)
    }
}

fn tool(found: Option<&FoundTool>, name: &str) -> Result<FoundTool, DumpError> {
    found
        .cloned()
        .ok_or_else(|| DumpError::Invalid(format!("{name} is needed for this restore")))
}

/// The restore's commands; `list` is the use-list file of a selection.
fn restore_commands(
    plan: &RestorePlan,
    spec: &ConnectionSpec,
    list: Option<&Path>,
) -> Result<Vec<ToolCommand>, DumpError> {
    if !plan.options.schemas.is_empty() && (plan.mysql || plan.kind == DumpKind::PlainSql) {
        return Err(DumpError::Invalid(
            "Schema-scoped restores require a PostgreSQL archive".to_owned(),
        ));
    }
    let client_name = if plan.mysql { "mysql" } else { "psql" };
    let mut commands = Vec::new();
    if plan.create {
        let client = tool(plan.client.as_ref(), client_name)?;
        commands.push(if plan.mysql {
            mysql_create_database(&client, spec, &plan.database)
        } else {
            psql_create_database(&client, spec, &plan.database)
        });
    }
    let target = spec.with_database(&plan.database);
    let options = &plan.options;
    commands.push(match plan.kind {
        DumpKind::PlainSql if plan.mysql => mysql_script(
            &tool(plan.client.as_ref(), client_name)?,
            &target,
            &plan.path,
            options,
        ),
        DumpKind::PlainSql => {
            let client = tool(plan.client.as_ref(), client_name)?;
            psql_script(&client, &target, &plan.path, options.stop_on_error)
        }
        kind => {
            let restore = tool(plan.archive_tool.as_ref(), "pg_restore")?;
            let archive = (plan.path.as_path(), kind == DumpKind::PgTar);
            pg_restore(&restore, &target, archive, options, list)?
        }
    });
    Ok(commands)
}

/// The restore's commands for a preview (no list is read or written).
pub(super) fn restore_preview(
    plan: &RestorePlan,
    spec: &ConnectionSpec,
) -> Result<Vec<ToolCommand>, DumpError> {
    let list = plan
        .options
        .selected
        .as_ref()
        .map(|_| Path::new(PREVIEW_LIST));
    restore_commands(plan, spec, list)
}

/// The restore's commands for running: reads the archive's contents for
/// the use-list and the progress total. Blocking.
pub(super) fn restore_built(plan: &RestorePlan, spec: &ConnectionSpec) -> Result<Built, DumpError> {
    let mut files = Vec::new();
    let mut total = None;
    if plan.kind.is_archive() {
        let restore = tool(plan.archive_tool.as_ref(), "pg_restore")?;
        let entries = parse_list(&capture(&pg_list(&restore, &plan.path))?);
        total = Some(entries.len() as u64);
        if let Some(selected) = &plan.options.selected {
            files.push(PrivateFile::new(
                "list",
                use_list(&entries, selected).as_bytes(),
            )?);
            total = Some(selected.len() as u64);
        }
    }
    let commands = restore_commands(plan, spec, files.first().map(PrivateFile::path))?;
    let total_bytes = (plan.kind == DumpKind::PlainSql)
        .then(|| std::fs::metadata(&plan.path).map(|meta| meta.len()).ok())
        .flatten();
    Ok(Built {
        commands,
        total,
        total_bytes,
        _files: files,
    })
}

/// A preview command as text: the loopback port, chosen when the job
/// starts, is shown as [`TUNNEL_PORT_MARK`].
pub(super) fn shown(command: &ToolCommand) -> String {
    let mut shown = command.clone();
    let conninfo_port = format!("port='{PREVIEW_PORT}'");
    let flag_port = format!("--port={PREVIEW_PORT}");
    for arg in &mut shown.args {
        let text = arg.to_string_lossy();
        if text.contains("hostaddr=") || text == flag_port {
            let replaced = text
                .replace(&conninfo_port, &format!("port='{TUNNEL_PORT_MARK}'"))
                .replace(&flag_port, &format!("--port={TUNNEL_PORT_MARK}"));
            *arg = replaced.into();
        }
    }
    shown.display()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoped_plain_scripts_are_refused_before_any_restore_command() {
        let spec = ConnectionSpec {
            address: openrows_dump::Address::Tcp {
                host: "localhost".into(),
                port: 5432,
            },
            user: "test-only".into(),
            database: "shop".into(),
            tls: openrows_dump::TlsSpec::default(),
            application_name: "test-only".into(),
            password_file: None,
        };
        for mysql in [false, true] {
            let plan = RestorePlan {
                kind: DumpKind::PlainSql,
                path: PathBuf::from("shop.sql"),
                database: "shop".into(),
                create: false,
                options: RestoreOptions {
                    schemas: vec!["public".into()],
                    ..RestoreOptions::default()
                },
                mysql,
                archive_tool: None,
                client: None,
            };
            assert!(restore_preview(&plan, &spec).is_err_and(|error| {
                error
                    .to_string()
                    .contains("Schema-scoped restores require a PostgreSQL archive")
            }));
        }
    }
}
