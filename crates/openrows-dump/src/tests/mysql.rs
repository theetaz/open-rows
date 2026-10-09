// SPDX-License-Identifier: Apache-2.0
use std::path::PathBuf;

use super::*;
use crate::command::PASSWORD_FILE_MARK;
use crate::conn::TlsSpec;
use crate::options::TableRef;
use crate::tools::ToolKind;
use crate::version::{ToolVersion, Version};

fn tool(kind: ToolKind, product: Product, major: u32) -> FoundTool {
    FoundTool {
        kind,
        path: PathBuf::from(format!("/usr/local/bin/{}", kind.label())),
        version: ToolVersion {
            product,
            version: Version {
                major,
                minor: 4,
                patch: 0,
            },
        },
    }
}

fn spec(mode: TlsMode) -> ConnectionSpec {
    ConnectionSpec {
        address: Address::Tcp {
            host: "db.example.com".to_owned(),
            port: 3306,
        },
        user: "app".to_owned(),
        database: "shop".to_owned(),
        tls: TlsSpec {
            mode,
            ca_file: Some(PathBuf::from("/certs/ca.pem")),
            ..TlsSpec::default()
        },
        application_name: "OpenRows".to_owned(),
        password_file: Some(PathBuf::from("/tmp/openrows-mysql-x1")),
    }
}

fn args(command: &ToolCommand) -> Vec<String> {
    command
        .args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
}

#[test]
fn mysqldump_with_every_option() -> Result<(), DumpError> {
    let options = BackupOptions {
        content: Content::SchemaOnly,
        tables: vec![TableRef {
            schema: "shop".to_owned(),
            name: "orders".to_owned(),
        }],
        single_transaction: true,
        routines: true,
        triggers: false,
        events: true,
        ..BackupOptions::default()
    };
    let dump = tool(ToolKind::MysqlDump, Product::Mysql, 8);
    let command = mysqldump(
        &dump,
        &spec(TlsMode::VerifyFull),
        &options,
        Path::new("/out/shop.sql"),
    )?;
    assert_eq!(
        args(&command),
        [
            "--defaults-extra-file=/tmp/openrows-mysql-x1",
            "--host=db.example.com",
            "--port=3306",
            "--protocol=TCP",
            "--user=app",
            "--ssl-mode=VERIFY_IDENTITY",
            "--ssl-ca=/certs/ca.pem",
            "--verbose",
            "--result-file=/out/shop.sql",
            "--no-tablespaces",
            "--single-transaction",
            "--routines",
            "--events",
            "--skip-triggers",
            "--no-data",
            "--set-gtid-purged=OFF",
            "--column-statistics=0",
            "shop",
            "orders",
        ]
    );
    assert!(command.env.is_empty());
    assert!(command.env_remove.contains(&"MYSQL_PWD"));
    assert!(command.display().starts_with(&format!(
        "/usr/local/bin/mysqldump --defaults-extra-file={PASSWORD_FILE_MARK} --host="
    )));
    Ok(())
}

#[test]
fn mariadb_clients_get_their_own_tls_flags() {
    let cases = [
        (TlsMode::Disable, vec!["--skip-ssl"]),
        (TlsMode::Prefer, vec!["--skip-ssl-verify-server-cert"]),
        (
            TlsMode::Require,
            vec!["--ssl", "--skip-ssl-verify-server-cert"],
        ),
        (
            TlsMode::VerifyFull,
            vec!["--ssl", "--ssl-verify-server-cert"],
        ),
    ];
    for (mode, expected) in cases {
        assert_eq!(
            tls_args(Product::Mariadb, mode, false),
            expected,
            "{mode:?}"
        );
    }
    assert_eq!(
        tls_args(Product::Mariadb, TlsMode::VerifyFull, true),
        ["--ssl", "--skip-ssl-verify-server-cert"]
    );
    assert_eq!(
        tls_args(Product::Mysql, TlsMode::VerifyFull, true),
        ["--ssl-mode=VERIFY_CA"]
    );
    assert_eq!(
        tls_args(Product::Mysql, TlsMode::Disable, false),
        ["--ssl-mode=DISABLED"]
    );
}

#[test]
fn forwarded_and_socket_connections() {
    let dump = tool(ToolKind::Mysql, Product::Mariadb, 11);
    let mut spec = spec(TlsMode::Disable);
    spec.password_file = None;
    spec.address = Address::Forwarded {
        port: 50111,
        server_host: "db.internal".to_owned(),
    };
    let script = mysql_script(
        &dump,
        &spec,
        Path::new("/in/a.sql"),
        &RestoreOptions::default(),
    );
    assert_eq!(
        args(&script),
        [
            "--host=127.0.0.1",
            "--port=50111",
            "--protocol=TCP",
            "--user=app",
            "--skip-ssl",
            "--force",
            "shop"
        ]
    );
    spec.address = Address::Socket("/tmp/mysql.sock".to_owned());
    let create = mysql_create_database(&dump, &spec, "new`db");
    assert_eq!(
        args(&create),
        [
            "--socket=/tmp/mysql.sock",
            "--protocol=SOCKET",
            "--user=app",
            "--execute=CREATE DATABASE `new``db`"
        ]
    );
}

#[test]
fn old_and_mariadb_dumps_skip_mysql_8_only_flags() -> Result<(), DumpError> {
    for (product, major) in [(Product::Mysql, 5), (Product::Mariadb, 11)] {
        let command = mysqldump(
            &tool(ToolKind::MysqlDump, product, major),
            &spec(TlsMode::Prefer),
            &BackupOptions::default(),
            Path::new("/o.sql"),
        )?;
        assert!(
            !args(&command)
                .iter()
                .any(|arg| arg.starts_with("--set-gtid"))
        );
    }
    Ok(())
}
