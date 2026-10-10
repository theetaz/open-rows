// SPDX-License-Identifier: Apache-2.0
//! `pg_dump`, `pg_restore` and `psql` argument vectors. The connection
//! travels as one libpq connection string (`--dbname`) without a password;
//! the password is read from the 0600 `PGPASSFILE`.
use std::path::Path;

use crate::command::{PG_CLEARED_ENV, ProgressKind, ToolCommand};
use crate::conn::{Address, ConnectionSpec, conninfo_value, pg_socket_dir};
use crate::discover::FoundTool;
use crate::error::DumpError;
use crate::options::{BackupOptions, Clean, Content, RestoreOptions};
use crate::version::Version;

/// First libpq release that accepts `sslrootcert=system`.
const SYSTEM_ROOTS_SINCE: u32 = 16;

/// The libpq connection string for `spec`, as a client of `client` reads it.
#[must_use]
pub fn conninfo(spec: &ConnectionSpec, client: Version) -> String {
    let mut parts: Vec<(&str, String)> = Vec::new();
    let socket = match &spec.address {
        Address::Tcp { host, port } => {
            parts.extend([("host", host.clone()), ("port", port.to_string())]);
            false
        }
        Address::Socket(path) => {
            let (dir, port) = pg_socket_dir(path);
            parts.push(("host", dir));
            parts.extend(port.map(|port| ("port", port.to_string())));
            true
        }
        Address::Forwarded { port, server_host } => {
            // libpq connects to `hostaddr` and checks the certificate against `host`.
            if !server_host.is_empty() {
                parts.push(("host", server_host.clone()));
            }
            parts.extend([
                ("hostaddr", "127.0.0.1".to_owned()),
                ("port", port.to_string()),
            ]);
            false
        }
    };
    parts.extend([
        ("user", spec.user.clone()),
        ("dbname", spec.database.clone()),
        ("application_name", spec.application_name.clone()),
    ]);
    if !socket {
        parts.extend(tls_parts(spec, client));
    }
    parts
        .iter()
        .map(|(key, value)| format!("{key}={}", conninfo_value(value)))
        .collect::<Vec<_>>()
        .join(" ")
}

fn tls_parts(spec: &ConnectionSpec, client: Version) -> Vec<(&'static str, String)> {
    let tls = &spec.tls;
    let mut parts = vec![("sslmode", tls.mode.libpq().to_owned())];
    let file = |path: &Path| path.to_string_lossy().into_owned();
    match &tls.ca_file {
        Some(ca) => parts.push(("sslrootcert", file(ca))),
        // The app checks against the system trust store; libpq only does so when told.
        None if tls.mode.verifies() && client.major >= SYSTEM_ROOTS_SINCE => {
            parts.push(("sslrootcert", "system".to_owned()));
        }
        None => {}
    }
    parts.extend(
        tls.client_cert
            .as_deref()
            .map(|cert| ("sslcert", file(cert))),
    );
    parts.extend(tls.client_key.as_deref().map(|key| ("sslkey", file(key))));
    parts
}

/// A command for `tool` that logs in as `spec` (password file, cleared
/// environment).
fn logged_in(tool: &FoundTool, spec: &ConnectionSpec, progress: ProgressKind) -> ToolCommand {
    let mut command = ToolCommand::new(&tool.path, progress);
    command.env_remove = PG_CLEARED_ENV.to_vec();
    if let Some(file) = &spec.password_file {
        command
            .env
            .push(("PGPASSFILE".to_owned(), file.clone().into_os_string()));
        command.password_file = Some(file.clone());
    }
    command
}

fn dbname(spec: &ConnectionSpec, tool: &FoundTool) -> String {
    format!("--dbname={}", conninfo(spec, tool.version.version))
}

/// A double-quoted identifier for `pg_dump` patterns (taken literally).
fn quoted(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// `pg_dump` writing `output` (a file, or a folder for the directory format).
///
/// # Errors
/// Invalid option combinations.
pub fn pg_dump(
    tool: &FoundTool,
    spec: &ConnectionSpec,
    options: &BackupOptions,
    output: &Path,
) -> Result<ToolCommand, DumpError> {
    options.validate()?;
    let mut command = logged_in(tool, spec, ProgressKind::PgDump);
    command
        .arg("--verbose")
        .arg("--no-password")
        .arg(format!("--format={}", options.format.letter()))
        .arg(format!("--file={}", output.display()));
    match options.content {
        Content::All => {}
        Content::SchemaOnly => _ = command.arg("--schema-only"),
        Content::DataOnly => _ = command.arg("--data-only"),
    }
    for schema in &options.schemas {
        command.arg(format!("--schema={}", quoted(schema)));
    }
    for table in &options.tables {
        let name = format!("{}.{}", quoted(&table.schema), quoted(&table.name));
        command.arg(format!("--table={name}"));
    }
    for table in &options.exclude_table_data {
        let name = format!("{}.{}", quoted(&table.schema), quoted(&table.name));
        command.arg(format!("--exclude-table-data={name}"));
    }
    flag(&mut command, options.no_owner, "--no-owner");
    flag(&mut command, options.no_privileges, "--no-privileges");
    if let Some(level) = options.compression {
        command.arg(format!("--compress={level}"));
    }
    command.arg(dbname(spec, tool));
    command.output = Some(output.to_path_buf());
    Ok(command)
}

fn flag(command: &mut ToolCommand, on: bool, name: &str) {
    if on {
        command.arg(name);
    }
}

/// `pg_restore` of `archive` into `spec`'s database; `use_list` restricts
/// it to the entries of a list file.
///
/// # Errors
/// Invalid option combinations.
pub fn pg_restore(
    tool: &FoundTool,
    spec: &ConnectionSpec,
    (archive, tar): (&Path, bool),
    options: &RestoreOptions,
    use_list: Option<&Path>,
) -> Result<ToolCommand, DumpError> {
    options.validate(tar)?;
    let mut command = logged_in(tool, spec, ProgressKind::PgRestore);
    command
        .arg("--verbose")
        .arg("--no-password")
        .arg(dbname(spec, tool));
    for schema in &options.schemas {
        command.arg(format!("--schema={schema}"));
    }
    flag(&mut command, options.clean != Clean::Keep, "--clean");
    flag(
        &mut command,
        options.clean == Clean::DropIfExists,
        "--if-exists",
    );
    flag(&mut command, options.no_owner, "--no-owner");
    flag(&mut command, options.no_privileges, "--no-privileges");
    flag(&mut command, options.stop_on_error, "--exit-on-error");
    if options.jobs > 1 {
        command.arg(format!("--jobs={}", options.jobs));
    }
    if let Some(list) = use_list {
        command.arg(format!("--use-list={}", list.display()));
    }
    command.arg(archive);
    Ok(command)
}

/// `pg_restore --list`: the archive's table of contents; no connection.
#[must_use]
pub fn pg_list(tool: &FoundTool, archive: &Path) -> ToolCommand {
    let mut command = ToolCommand::new(&tool.path, ProgressKind::Script);
    command.arg("--list").arg(archive);
    command
}

/// `psql` running the SQL script `script` (piped in) on `spec`'s database.
#[must_use]
pub fn psql_script(
    tool: &FoundTool,
    spec: &ConnectionSpec,
    script: &Path,
    stop_on_error: bool,
) -> ToolCommand {
    let mut command = logged_in(tool, spec, ProgressKind::Script);
    command
        .arg("--no-psqlrc")
        .arg("--no-password")
        .arg(dbname(spec, tool));
    if stop_on_error {
        command.arg("--set=ON_ERROR_STOP=1");
    }
    command.stdin = Some(script.to_path_buf());
    command
}

/// `psql` creating database `name`, logged in to `spec`'s database.
#[must_use]
pub fn psql_create_database(tool: &FoundTool, spec: &ConnectionSpec, name: &str) -> ToolCommand {
    let mut command = logged_in(tool, spec, ProgressKind::Script);
    command
        .arg("--no-psqlrc")
        .arg("--no-password")
        .arg(dbname(spec, tool))
        .arg("--set=ON_ERROR_STOP=1")
        .arg(format!("--command=CREATE DATABASE {}", quoted(name)));
    command
}

#[cfg(test)]
#[path = "tests/pg.rs"]
mod tests;
