// SPDX-License-Identifier: Apache-2.0
//! `mysqldump` and `mysql` (or `mariadb-dump` and `mariadb`) round trips
//! against a real server, with the container's own tools (`docker exec`)
//! and with locally installed tools when they exist.
//!
//! Ignored by `just test`; `just test-mysql [images]` starts throwaway
//! containers and runs them with `--run-ignored only`.
mod support;

use std::path::Path;

use openrows_dump::{
    Address, BackupOptions, ConnectionSpec, Content, FoundTool, PrivateFile, RestoreOptions,
    TableRef, TlsMode, TlsSpec, ToolKind, choose, discover, mysql_create_database, mysql_script,
    mysqldump, search_dirs,
};
use support::{
    TestResult, container_tool, copy_out, docker, in_container, succeed, unique, variable,
};

const RECIPE: &str = "just test-mysql";

struct Server {
    container: String,
    password: String,
}

impl Server {
    fn from_env() -> TestResult<Self> {
        Ok(Self {
            container: variable("OPENROWS_TEST_MYSQL_CONTAINER", RECIPE)?,
            password: variable("OPENROWS_TEST_MYSQL_PASSWORD", RECIPE)?,
        })
    }

    /// The container's first available name among `names`.
    fn tool(&self, kind: ToolKind) -> TestResult<FoundTool> {
        let probe = kind
            .names()
            .iter()
            .map(|name| format!("command -v {name}"))
            .collect::<Vec<_>>()
            .join(" || ");
        let path = docker(["exec", &self.container, "sh", "-c", &probe])?;
        let name = path.rsplit('/').next().ok_or("no tool")?;
        container_tool(&self.container, kind, name)
    }

    /// Run `sql` inside the container as root.
    fn sql(&self, sql: &str) -> TestResult<String> {
        let client = self.tool(ToolKind::Mysql)?;
        let password = format!("--password={}", self.password);
        let program = client.path.to_string_lossy().into_owned();
        docker([
            "exec",
            &self.container,
            &program,
            "--user=root",
            &password,
            "--batch",
            "--skip-column-names",
            "--execute",
            sql,
        ])
    }

    fn seeded(&self, name: &str) -> TestResult {
        self.sql(&format!(
            "CREATE DATABASE {name};
             CREATE TABLE {name}.items (id int PRIMARY KEY, name varchar(40));
             CREATE TABLE {name}.skipped (id int);
             INSERT INTO {name}.items VALUES (1, 'one'), (2, 'two'), (3, 'three');"
        ))?;
        Ok(())
    }
}

fn spec(address: Address, database: &str, password_file: &PrivateFile) -> ConnectionSpec {
    ConnectionSpec {
        address,
        user: "root".to_owned(),
        database: database.to_owned(),
        tls: TlsSpec {
            mode: TlsMode::Prefer,
            ..TlsSpec::default()
        },
        application_name: "OpenRows".to_owned(),
        password_file: Some(password_file.path().to_path_buf()),
    }
}

fn round_trip(
    server: &Server,
    (dump_tool, client): (&FoundTool, &FoundTool),
    address: &Address,
    docker_exec: bool,
) -> TestResult {
    let (source, target) = (unique("dump_src"), unique("dump_copy"));
    server.seeded(&source)?;
    let password = PrivateFile::mysql_options(&server.password)?;
    let host = tempfile::tempdir()?;
    let local = host.path().join("dump.sql");
    let output = if docker_exec {
        format!("/tmp/{source}.sql")
    } else {
        local.to_string_lossy().into_owned()
    };
    let options = BackupOptions {
        content: Content::All,
        tables: vec![TableRef {
            schema: source.clone(),
            name: "items".to_owned(),
        }],
        ..BackupOptions::default()
    };
    let dump = mysqldump(
        dump_tool,
        &spec(address.clone(), &source, &password),
        &options,
        Path::new(&output),
    )?;
    assert!(
        dump.display()
            .contains("--defaults-extra-file=<password file>")
    );
    let wrap = |command| {
        if docker_exec {
            in_container(&command, &server.container, &[])
        } else {
            Ok(command)
        }
    };
    assert_eq!(succeed(&wrap(dump)?)?, 1, "one table dumped");
    if docker_exec {
        copy_out(&server.container, &output, &local)?;
    }
    succeed(&wrap(mysql_create_database(
        client,
        &spec(address.clone(), "", &password),
        &target,
    ))?)?;
    let options = RestoreOptions {
        stop_on_error: true,
        ..RestoreOptions::default()
    };
    succeed(&wrap(mysql_script(
        client,
        &spec(address.clone(), &target, &password),
        &local,
        &options,
    ))?)?;
    assert_eq!(
        server.sql(&format!("SELECT count(*) FROM {target}.items"))?,
        "3"
    );
    let skipped = server.sql(&format!(
        "SELECT count(*) FROM information_schema.tables WHERE table_schema = '{target}' AND table_name = 'skipped'"
    ))?;
    assert_eq!(skipped, "0");
    Ok(())
}

#[test]
#[ignore = "needs MySQL or MariaDB; run with `just test-mysql`"]
fn round_trip_with_the_containers_tools() -> TestResult {
    let server = Server::from_env()?;
    let dump_tool = server.tool(ToolKind::MysqlDump)?;
    let client = server.tool(ToolKind::Mysql)?;
    let inside = Address::Tcp {
        host: "127.0.0.1".to_owned(),
        port: 3306,
    };
    round_trip(&server, (&dump_tool, &client), &inside, true)
}

#[test]
#[ignore = "needs MySQL or MariaDB; run with `just test-mysql`"]
fn round_trip_with_local_tools_when_installed() -> TestResult {
    let server = Server::from_env()?;
    let path = std::env::var_os("PATH");
    let dirs = search_dirs(path.as_deref());
    let dumps = discover(ToolKind::MysqlDump, &dirs);
    let clients = discover(ToolKind::Mysql, &dirs);
    let (Some(dump_tool), Some(client)) = (choose(&dumps, None), choose(&clients, None)) else {
        // No local client tools: the container test covers the commands.
        return Ok(());
    };
    let port: u16 = variable("OPENROWS_TEST_MYSQL_PORT", RECIPE)?.parse()?;
    let local = Address::Tcp {
        host: "127.0.0.1".to_owned(),
        port,
    };
    round_trip(&server, (dump_tool, client), &local, false)
}
