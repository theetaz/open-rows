// SPDX-License-Identifier: Apache-2.0
use std::path::PathBuf;

use super::*;
use crate::command::PASSWORD_FILE_MARK;
use crate::conn::{TlsMode, TlsSpec};
use crate::options::{PgFormat, TableRef};
use crate::tools::ToolKind;
use crate::version::{Product, ToolVersion};

fn tool(kind: ToolKind, major: u32) -> FoundTool {
    FoundTool {
        kind,
        path: PathBuf::from(format!("/opt/homebrew/opt/libpq/bin/{}", kind.label())),
        version: ToolVersion {
            product: Product::Postgres,
            version: Version {
                major,
                minor: 4,
                patch: 0,
            },
        },
    }
}

fn spec() -> ConnectionSpec {
    ConnectionSpec {
        address: Address::Tcp {
            host: "db.example.com".to_owned(),
            port: 5432,
        },
        user: "app".to_owned(),
        database: "shop".to_owned(),
        tls: TlsSpec {
            mode: TlsMode::VerifyFull,
            ca_file: Some(PathBuf::from("/certs/ca.pem")),
            client_cert: None,
            client_key: None,
        },
        application_name: "OpenRows Backup".to_owned(),
        password_file: Some(PathBuf::from("/tmp/openrows-pgpass-x1")),
    }
}

fn args(command: &ToolCommand) -> Vec<String> {
    command
        .args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
}

const CONNINFO: &str = "--dbname=host='db.example.com' port='5432' user='app' dbname='shop' \
    application_name='OpenRows Backup' sslmode='verify-full' sslrootcert='/certs/ca.pem'";

#[test]
fn custom_format_backup_with_every_option() -> Result<(), DumpError> {
    let options = BackupOptions {
        format: PgFormat::Custom,
        content: Content::SchemaOnly,
        schemas: vec!["sales".to_owned()],
        tables: vec![TableRef {
            schema: "public".to_owned(),
            name: "Order \"Items\"".to_owned(),
        }],
        no_owner: true,
        no_privileges: true,
        compression: Some(6),
        ..BackupOptions::default()
    };
    let command = pg_dump(
        &tool(ToolKind::PgDump, 17),
        &spec(),
        &options,
        Path::new("/out/shop.dump"),
    )?;
    assert_eq!(
        args(&command),
        [
            "--verbose",
            "--no-password",
            "--format=c",
            "--file=/out/shop.dump",
            "--schema-only",
            "--schema=\"sales\"",
            "--table=\"public\".\"Order \"\"Items\"\"\"",
            "--no-owner",
            "--no-privileges",
            "--compress=6",
            CONNINFO,
        ]
    );
    assert_eq!(command.output.as_deref(), Some(Path::new("/out/shop.dump")));
    assert_eq!(command.progress, ProgressKind::PgDump);
    Ok(())
}

#[test]
fn the_password_only_travels_in_the_password_file() -> Result<(), DumpError> {
    let command = pg_dump(
        &tool(ToolKind::PgDump, 17),
        &spec(),
        &BackupOptions::default(),
        Path::new("/out/x.dump"),
    )?;
    assert_eq!(
        command.env,
        [(
            "PGPASSFILE".to_owned(),
            std::ffi::OsString::from("/tmp/openrows-pgpass-x1")
        )]
    );
    assert!(command.env_remove.contains(&"PGPASSWORD"));
    assert!(!args(&command).iter().any(|arg| arg.contains("password=")));
    let display = command.display();
    assert!(display.starts_with(&format!(
        "PGPASSFILE={PASSWORD_FILE_MARK} /opt/homebrew/opt/libpq/bin/pg_dump --verbose"
    )));
    assert!(!display.contains("openrows-pgpass"));
    assert!(display.contains("'--dbname=host='\\''db.example.com'\\''"));
    Ok(())
}

#[test]
fn every_format_maps_to_its_letter_and_options_are_checked() {
    let letters = [
        (PgFormat::Plain, "--format=p"),
        (PgFormat::Directory, "--format=d"),
        (PgFormat::Tar, "--format=t"),
    ];
    for (format, flag) in letters {
        let options = BackupOptions {
            format,
            content: Content::DataOnly,
            ..BackupOptions::default()
        };
        let command = pg_dump(
            &tool(ToolKind::PgDump, 17),
            &spec(),
            &options,
            Path::new("/o"),
        );
        let args = command.map(|command| args(&command)).unwrap_or_default();
        assert!(args.contains(&flag.to_owned()) && args.contains(&"--data-only".to_owned()));
    }
    let tar = BackupOptions {
        format: PgFormat::Tar,
        compression: Some(5),
        ..BackupOptions::default()
    };
    assert!(pg_dump(&tool(ToolKind::PgDump, 17), &spec(), &tar, Path::new("/o")).is_err());
    let level = BackupOptions {
        compression: Some(10),
        ..BackupOptions::default()
    };
    assert!(
        pg_dump(
            &tool(ToolKind::PgDump, 17),
            &spec(),
            &level,
            Path::new("/o")
        )
        .is_err()
    );
}

#[test]
fn forwarded_connections_check_the_certificate_against_the_real_host() {
    let mut spec = spec();
    spec.address = Address::Forwarded {
        port: 61234,
        server_host: "db.internal".to_owned(),
    };
    spec.tls.ca_file = None;
    let info = conninfo(&spec, tool(ToolKind::PgDump, 17).version.version);
    assert!(info.starts_with("host='db.internal' hostaddr='127.0.0.1' port='61234' "));
    assert!(info.ends_with("sslmode='verify-full' sslrootcert='system'"));
    // libpq before 16 has no system trust store option.
    let old = conninfo(&spec, tool(ToolKind::PgDump, 15).version.version);
    assert!(old.ends_with("sslmode='verify-full'"));
}

#[test]
fn sockets_skip_tls_and_split_the_socket_file() {
    let mut spec = spec();
    spec.address = Address::Socket("/tmp/.s.PGSQL.5433".to_owned());
    spec.tls.client_key = Some(PathBuf::from("/k.pem"));
    let info = conninfo(&spec, tool(ToolKind::Psql, 17).version.version);
    assert_eq!(
        info,
        "host='/tmp' port='5433' user='app' dbname='shop' application_name='OpenRows Backup'"
    );
    spec.address = Address::Socket("/var/run/postgresql/".to_owned());
    assert!(
        conninfo(&spec, tool(ToolKind::Psql, 17).version.version)
            .starts_with("host='/var/run/postgresql' user=")
    );
}

#[test]
fn quotes_in_values_are_escaped() {
    let mut spec = spec();
    spec.user = r"o'brien\x".to_owned();
    spec.tls = TlsSpec {
        mode: TlsMode::Prefer,
        ..TlsSpec::default()
    };
    let info = conninfo(&spec, tool(ToolKind::Psql, 17).version.version);
    assert!(info.contains(r"user='o\'brien\\x'"));
    assert!(info.ends_with("sslmode='prefer'"));
}

#[test]
fn restore_lists_and_parallel_jobs() -> Result<(), DumpError> {
    let options = RestoreOptions {
        clean: Clean::DropIfExists,
        no_owner: true,
        no_privileges: true,
        jobs: 4,
        stop_on_error: true,
        selected: Some(vec![1, 2]),
    };
    let command = pg_restore(
        &tool(ToolKind::PgRestore, 17),
        &spec(),
        (Path::new("/in/shop.dump"), false),
        &options,
        Some(Path::new("/tmp/list")),
    )?;
    let args = args(&command);
    assert_eq!(&args[..2], ["--verbose", "--no-password"]);
    assert_eq!(
        &args[3..],
        [
            "--clean",
            "--if-exists",
            "--no-owner",
            "--no-privileges",
            "--exit-on-error",
            "--jobs=4",
            "--use-list=/tmp/list",
            "/in/shop.dump",
        ]
    );
    let tar = pg_restore(
        &tool(ToolKind::PgRestore, 17),
        &spec(),
        (Path::new("/in/x.tar"), true),
        &options,
        None,
    );
    assert!(tar.is_err());
    let none = RestoreOptions {
        selected: Some(Vec::new()),
        ..RestoreOptions::default()
    };
    assert!(none.validate(false).is_err());
    Ok(())
}

#[test]
fn psql_runs_scripts_from_stdin_and_creates_databases() {
    let psql = tool(ToolKind::Psql, 17);
    let script = psql_script(&psql, &spec(), Path::new("/in/shop.sql"), true);
    assert_eq!(script.stdin.as_deref(), Some(Path::new("/in/shop.sql")));
    assert!(args(&script).contains(&"--set=ON_ERROR_STOP=1".to_owned()));
    assert!(script.display().ends_with("< /in/shop.sql"));
    let create = psql_create_database(&psql, &spec(), "new \"db\"");
    assert_eq!(
        args(&create).last().map(String::as_str),
        Some("--command=CREATE DATABASE \"new \"\"db\"\"\"")
    );
    let list = pg_list(&tool(ToolKind::PgRestore, 17), Path::new("/in/a.dump"));
    assert_eq!(args(&list), ["--list", "/in/a.dump"]);
    assert!(list.env.is_empty());
}
