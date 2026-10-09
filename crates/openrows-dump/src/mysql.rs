// SPDX-License-Identifier: Apache-2.0
//! `mysqldump`/`mariadb-dump` and `mysql`/`mariadb` argument vectors. The
//! password is only in the 0600 `--defaults-extra-file`, which must be the
//! first argument.
use std::path::Path;

use crate::command::{MYSQL_CLEARED_ENV, ProgressKind, ToolCommand};
use crate::conn::{Address, ConnectionSpec, TlsMode};
use crate::discover::FoundTool;
use crate::error::DumpError;
use crate::options::{BackupOptions, Content, RestoreOptions};
use crate::version::Product;

/// A command for `tool` that logs in as `spec`.
fn logged_in(tool: &FoundTool, spec: &ConnectionSpec, progress: ProgressKind) -> ToolCommand {
    let mut command = ToolCommand::new(&tool.path, progress);
    command.env_remove = MYSQL_CLEARED_ENV.to_vec();
    if let Some(file) = &spec.password_file {
        command.arg(format!("--defaults-extra-file={}", file.display()));
        command.password_file = Some(file.clone());
    }
    let socket = match &spec.address {
        Address::Tcp { host, port } => {
            command
                .arg(format!("--host={host}"))
                .arg(format!("--port={port}"));
            false
        }
        Address::Forwarded { port, .. } => {
            command
                .arg("--host=127.0.0.1")
                .arg(format!("--port={port}"));
            false
        }
        Address::Socket(path) => {
            command.arg(format!("--socket={path}"));
            true
        }
    };
    command.arg(if socket {
        "--protocol=SOCKET"
    } else {
        "--protocol=TCP"
    });
    command.arg(format!("--user={}", spec.user));
    if !socket {
        let forwarded = matches!(spec.address, Address::Forwarded { .. });
        for arg in tls_args(tool.version.product, spec.tls.mode, forwarded) {
            command.arg(arg);
        }
        tls_files(&mut command, spec);
    }
    command
}

/// TLS flags of the client's dialect. Through the loopback forward the
/// host name cannot match the certificate, so `verify-full` checks the
/// chain only.
fn tls_args(client: Product, mode: TlsMode, forwarded: bool) -> Vec<&'static str> {
    let mode = match mode {
        TlsMode::VerifyFull if forwarded => TlsMode::VerifyCa,
        other => other,
    };
    if client == Product::Mariadb {
        return match mode {
            TlsMode::Disable => vec!["--skip-ssl"],
            TlsMode::Allow | TlsMode::Prefer => vec!["--skip-ssl-verify-server-cert"],
            TlsMode::Require | TlsMode::VerifyCa => vec!["--ssl", "--skip-ssl-verify-server-cert"],
            TlsMode::VerifyFull => vec!["--ssl", "--ssl-verify-server-cert"],
        };
    }
    vec![match mode {
        TlsMode::Disable => "--ssl-mode=DISABLED",
        TlsMode::Allow | TlsMode::Prefer => "--ssl-mode=PREFERRED",
        TlsMode::Require => "--ssl-mode=REQUIRED",
        TlsMode::VerifyCa => "--ssl-mode=VERIFY_CA",
        TlsMode::VerifyFull => "--ssl-mode=VERIFY_IDENTITY",
    }]
}

fn tls_files(command: &mut ToolCommand, spec: &ConnectionSpec) {
    let tls = &spec.tls;
    if tls.mode == TlsMode::Disable {
        return;
    }
    let files = [
        ("--ssl-ca", &tls.ca_file),
        ("--ssl-cert", &tls.client_cert),
        ("--ssl-key", &tls.client_key),
    ];
    for (flag, file) in files {
        if let Some(path) = file {
            command.arg(format!("{flag}={}", path.display()));
        }
    }
}

/// `mysqldump` of `spec`'s database (or some of its tables) into `output`.
///
/// # Errors
/// Invalid option combinations.
pub fn mysqldump(
    tool: &FoundTool,
    spec: &ConnectionSpec,
    options: &BackupOptions,
    output: &Path,
) -> Result<ToolCommand, DumpError> {
    options.validate()?;
    let mut command = logged_in(tool, spec, ProgressKind::MysqlDump);
    command
        .arg("--verbose")
        .arg(format!("--result-file={}", output.display()))
        // Tablespaces need the PROCESS privilege and rarely restore elsewhere.
        .arg("--no-tablespaces");
    let flags = [
        (options.single_transaction, "--single-transaction"),
        (options.routines, "--routines"),
        (options.events, "--events"),
        (!options.triggers, "--skip-triggers"),
        (options.content == Content::SchemaOnly, "--no-data"),
        (options.content == Content::DataOnly, "--no-create-info"),
    ];
    for (on, flag) in flags {
        if on {
            command.arg(flag);
        }
    }
    if tool.version.product == Product::Mysql && tool.version.version.major >= 8 {
        // GTID statements and histogram queries break restores into other
        // servers and MariaDB.
        command
            .arg("--set-gtid-purged=OFF")
            .arg("--column-statistics=0");
    }
    command.arg(&spec.database);
    for table in &options.tables {
        command.arg(&table.name);
    }
    command.output = Some(output.to_path_buf());
    Ok(command)
}

/// `mysql` running the SQL script `script` (piped in) on `spec`'s database.
#[must_use]
pub fn mysql_script(
    tool: &FoundTool,
    spec: &ConnectionSpec,
    script: &Path,
    options: &RestoreOptions,
) -> ToolCommand {
    let mut command = logged_in(tool, spec, ProgressKind::Script);
    if !options.stop_on_error {
        command.arg("--force");
    }
    command.arg(&spec.database);
    command.stdin = Some(script.to_path_buf());
    command
}

/// `mysql` creating database `name`.
#[must_use]
pub fn mysql_create_database(tool: &FoundTool, spec: &ConnectionSpec, name: &str) -> ToolCommand {
    let mut command = logged_in(tool, spec, ProgressKind::Script);
    let quoted = format!("`{}`", name.replace('`', "``"));
    command.arg(format!("--execute=CREATE DATABASE {quoted}"));
    command
}

#[cfg(test)]
#[path = "tests/mysql.rs"]
mod tests;
