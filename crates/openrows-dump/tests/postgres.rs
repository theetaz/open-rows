// SPDX-License-Identifier: Apache-2.0
//! Backup and restore round trips against a real `PostgreSQL` server: with
//! the container's own `pg_dump`, `pg_restore` and `psql` (`docker exec`),
//! and with locally installed tools when one is new enough.
//!
//! Ignored by `just test`; `just test-pg [version]` starts a throwaway
//! container and runs them with `--run-ignored only`.
mod support;

use std::path::{Path, PathBuf};

use openrows_dump::{
    Address, BackupOptions, Clean, ConnectionSpec, FoundTool, PgFormat, PrivateFile, Product,
    RestoreOptions, ServerVersion, TlsMode, TlsSpec, ToolKind, Version, capture, choose,
    compatibility, discover, parse_list, pg_dump, pg_list, pg_restore, psql_create_database,
    psql_script, search_dirs, use_list,
};
use support::{
    TestResult, container_tool, copy_out, docker, in_container, succeed, unique, variable,
};

const RECIPE: &str = "just test-pg";

struct Server {
    container: String,
    password: String,
}

impl Server {
    fn from_env() -> TestResult<Self> {
        Ok(Self {
            container: variable("OPENROWS_TEST_PG_CONTAINER", RECIPE)?,
            password: variable("OPENROWS_TEST_PG_PASSWORD", RECIPE)?,
        })
    }

    /// `psql -tAc sql` inside the container as `postgres`.
    fn sql(&self, database: &str, sql: &str) -> TestResult<String> {
        docker([
            "exec",
            &self.container,
            "psql",
            "-U",
            "postgres",
            "-d",
            database,
            "-v",
            "ON_ERROR_STOP=1",
            "-tAc",
            sql,
        ])
    }

    fn version(&self) -> TestResult<ServerVersion> {
        let text = self.sql("postgres", "SHOW server_version")?;
        Ok(ServerVersion {
            product: Product::Postgres,
            version: Version::parse(&text).ok_or("unparsable server version")?,
        })
    }

    /// A database holding a table, rows and a view.
    fn seeded(&self, name: &str) -> TestResult {
        self.sql("postgres", &format!("CREATE DATABASE {name}"))?;
        self.sql(
            name,
            "CREATE TABLE items (id int PRIMARY KEY, name text);
             INSERT INTO items SELECT g, 'item ' || g FROM generate_series(1, 250) g;
             CREATE VIEW cheap AS SELECT * FROM items WHERE id < 10;",
        )?;
        Ok(())
    }

    fn count(&self, database: &str) -> TestResult<String> {
        self.sql(database, "SELECT count(*) FROM items")
    }
}

fn spec(address: Address, database: &str, password_file: &PrivateFile) -> ConnectionSpec {
    ConnectionSpec {
        address,
        user: "postgres".to_owned(),
        database: database.to_owned(),
        tls: TlsSpec {
            mode: TlsMode::Prefer,
            ..TlsSpec::default()
        },
        application_name: "OpenRows Backup test".to_owned(),
        password_file: Some(password_file.path().to_path_buf()),
    }
}

#[test]
#[ignore = "needs PostgreSQL; run with `just test-pg`"]
fn custom_archive_round_trip_with_the_containers_tools() -> TestResult {
    let server = Server::from_env()?;
    let c = server.container.as_str();
    let (source, target) = (unique("dump_src"), unique("dump_custom"));
    server.seeded(&source)?;
    let password = PrivateFile::pgpass(&server.password)?;
    let inside = Address::Tcp {
        host: "127.0.0.1".to_owned(),
        port: 5432,
    };
    let dump_tool = container_tool(c, ToolKind::PgDump, "pg_dump")?;
    assert_eq!(
        compatibility(ToolKind::PgDump, dump_tool.version, Some(server.version()?)),
        None
    );
    let archive = format!("/tmp/{source}.dump");
    let dump = pg_dump(
        &dump_tool,
        &spec(inside.clone(), &source, &password),
        &BackupOptions::default(),
        Path::new(&archive),
    )?;
    assert!(dump.display().contains("PGPASSFILE=<password file>"));
    assert!(
        succeed(&in_container(&dump, c, &[])?)? >= 1,
        "no table counted"
    );

    let restore_tool = container_tool(c, ToolKind::PgRestore, "pg_restore")?;
    let listed = capture(&in_container(
        &pg_list(&restore_tool, Path::new(&archive)),
        c,
        &[],
    )?)?;
    let entries = parse_list(&listed);
    assert!(
        entries
            .iter()
            .any(|entry| entry.label.starts_with("TABLE public items"))
    );

    let psql = container_tool(c, ToolKind::Psql, "psql")?;
    succeed(&in_container(
        &psql_create_database(&psql, &spec(inside.clone(), "postgres", &password), &target),
        c,
        &[],
    )?)?;
    // Restore everything but the view, through a use-list.
    let keep: Vec<u32> = entries
        .iter()
        .filter(|entry| !entry.label.starts_with("VIEW"))
        .map(|entry| entry.id)
        .collect();
    let list = PrivateFile::new("list", use_list(&entries, &keep).as_bytes())?;
    let options = RestoreOptions {
        clean: Clean::DropIfExists,
        stop_on_error: true,
        jobs: 2,
        selected: Some(keep),
        ..RestoreOptions::default()
    };
    let restore = pg_restore(
        &restore_tool,
        &spec(inside, &target, &password),
        (Path::new(&archive), false),
        &options,
        Some(list.path()),
    )?;
    assert!(succeed(&in_container(&restore, c, &[list.path()])?)? > 0);
    assert_eq!(server.count(&target)?, "250");
    let views = server.sql(
        &target,
        "SELECT count(*) FROM pg_views WHERE viewname = 'cheap'",
    )?;
    assert_eq!(views, "0");
    Ok(())
}

#[test]
#[ignore = "needs PostgreSQL; run with `just test-pg`"]
fn plain_sql_round_trip_through_psql_on_stdin() -> TestResult {
    let server = Server::from_env()?;
    let c = server.container.as_str();
    let (source, target) = (unique("dump_plain_src"), unique("dump_plain"));
    server.seeded(&source)?;
    let password = PrivateFile::pgpass(&server.password)?;
    let inside = Address::Tcp {
        host: "127.0.0.1".to_owned(),
        port: 5432,
    };
    let script = format!("/tmp/{source}.sql");
    let options = BackupOptions {
        format: PgFormat::Plain,
        no_owner: true,
        no_privileges: true,
        ..BackupOptions::default()
    };
    let dump_tool = container_tool(c, ToolKind::PgDump, "pg_dump")?;
    let dump = pg_dump(
        &dump_tool,
        &spec(inside.clone(), &source, &password),
        &options,
        Path::new(&script),
    )?;
    succeed(&in_container(&dump, c, &[])?)?;
    let host = tempfile::tempdir()?;
    let local = host.path().join("dump.sql");
    copy_out(c, &script, &local)?;

    let psql = container_tool(c, ToolKind::Psql, "psql")?;
    let maintenance = spec(inside.clone(), "postgres", &password);
    succeed(&in_container(
        &psql_create_database(&psql, &maintenance, &target),
        c,
        &[],
    )?)?;
    let run = psql_script(&psql, &spec(inside, &target, &password), &local, true);
    succeed(&in_container(&run, c, &[])?)?;
    assert_eq!(server.count(&target)?, "250");
    Ok(())
}

/// The installed tools this machine has, newest matching first.
fn local_tools(kind: ToolKind) -> Vec<FoundTool> {
    let path = std::env::var_os("PATH");
    discover(kind, &search_dirs(path.as_deref()))
}

#[test]
#[ignore = "needs PostgreSQL; run with `just test-pg`"]
#[expect(
    clippy::print_stderr,
    reason = "says why the local-tool test did nothing"
)]
fn local_tools_round_trip_with_verified_tls_through_a_forwarded_address() -> TestResult {
    let server = Server::from_env()?;
    let version = server.version()?;
    let dumps = local_tools(ToolKind::PgDump);
    let Some(dump_tool) = choose(&dumps, Some(version))
        .filter(|tool| compatibility(ToolKind::PgDump, tool.version, Some(version)).is_none())
    else {
        for tool in &dumps {
            // Older local copies must be flagged rather than tried.
            assert!(compatibility(ToolKind::PgDump, tool.version, Some(version)).is_some());
        }
        eprintln!(
            "SKIP: no local pg_dump {} or newer; the docker exec tests cover the tools",
            version.version.major
        );
        return Ok(());
    };
    let restores = local_tools(ToolKind::PgRestore);
    let psqls = local_tools(ToolKind::Psql);
    let restore_tool = choose(&restores, Some(version)).ok_or("pg_restore missing")?;
    let psql = choose(&psqls, Some(version)).ok_or("psql missing")?;
    let tls_dir = PathBuf::from(variable("OPENROWS_TEST_PG_TLS_DIR", RECIPE)?);
    let port: u16 = variable("OPENROWS_TEST_PG_PORT", RECIPE)?.parse()?;
    let (source, target) = (unique("dump_local_src"), unique("dump_local"));
    server.seeded(&source)?;
    let password = PrivateFile::pgpass(&server.password)?;
    // The certificate names `localhost`: libpq connects to 127.0.0.1 and
    // checks `host`, exactly as for the app's tunnel endpoint.
    let address = Address::Forwarded {
        port,
        server_host: "localhost".to_owned(),
    };
    let verified = |database: &str| {
        let mut spec = spec(address.clone(), database, &password);
        spec.tls = TlsSpec {
            mode: TlsMode::VerifyFull,
            ca_file: Some(tls_dir.join("ca.crt")),
            ..TlsSpec::default()
        };
        spec
    };
    let out = tempfile::tempdir()?;
    let archive = out.path().join("local.tar");
    let options = BackupOptions {
        format: PgFormat::Tar,
        ..BackupOptions::default()
    };
    let objects = succeed(&pg_dump(dump_tool, &verified(&source), &options, &archive)?)?;
    assert!(objects >= 1);
    succeed(&psql_create_database(psql, &verified("postgres"), &target))?;
    let restore = pg_restore(
        restore_tool,
        &verified(&target),
        (&archive, true),
        &RestoreOptions::default(),
        None,
    )?;
    succeed(&restore)?;
    assert_eq!(server.count(&target)?, "250");
    Ok(())
}

#[test]
#[ignore = "needs PostgreSQL; run with `just test-pg`"]
fn selected_tables_and_schemas_restore_only_the_selected_objects() -> TestResult {
    let server = Server::from_env()?;
    let source = unique("scope_src");
    server.seeded(&source)?;
    server.sql(&source, "CREATE TABLE omitted (id int); CREATE SCHEMA selected; CREATE TABLE selected.kept (id int); CREATE SCHEMA skipped; CREATE TABLE skipped.omitted (id int)")?;
    let password = PrivateFile::pgpass(&server.password)?;
    let address = Address::Tcp {
        host: "127.0.0.1".into(),
        port: 5432,
    };
    let dump_tool = container_tool(&server.container, ToolKind::PgDump, "pg_dump")?;
    let restore_tool = container_tool(&server.container, ToolKind::PgRestore, "pg_restore")?;
    let selections = [
        BackupOptions {
            tables: vec![openrows_dump::TableRef {
                schema: "public".into(),
                name: "items".into(),
            }],
            ..BackupOptions::default()
        },
        BackupOptions {
            schemas: vec!["selected".into()],
            ..BackupOptions::default()
        },
        BackupOptions::default(),
    ];
    for (index, options) in selections.into_iter().enumerate() {
        let target = unique(&format!("scope_dst_{index}"));
        server.sql("postgres", &format!("CREATE DATABASE {target}"))?;
        let archive = format!("/tmp/{target}.dump");
        let dump = pg_dump(
            &dump_tool,
            &spec(address.clone(), &source, &password),
            &options,
            Path::new(&archive),
        )?;
        succeed(&in_container(&dump, &server.container, &[])?)?;
        let restore_options = if index == 2 {
            server.sql(&target, "CREATE SCHEMA selected")?;
            RestoreOptions {
                schemas: vec!["selected".into()],
                ..RestoreOptions::default()
            }
        } else {
            RestoreOptions::default()
        };
        let restore = pg_restore(
            &restore_tool,
            &spec(address.clone(), &target, &password),
            (Path::new(&archive), false),
            &restore_options,
            None,
        )?;
        succeed(&in_container(&restore, &server.container, &[])?)?;
        let tables = server.sql(&target, "SELECT schemaname || '.' || tablename FROM pg_tables WHERE schemaname NOT IN ('pg_catalog', 'information_schema') ORDER BY 1")?;
        assert_eq!(
            tables,
            if index == 0 {
                "public.items"
            } else {
                "selected.kept"
            }
        );
    }
    Ok(())
}
