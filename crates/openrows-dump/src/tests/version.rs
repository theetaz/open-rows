// SPDX-License-Identifier: Apache-2.0
use super::*;

fn version(major: u32, minor: u32, patch: u32) -> Version {
    Version {
        major,
        minor,
        patch,
    }
}

fn tool(product: Product, major: u32) -> ToolVersion {
    ToolVersion {
        product,
        version: version(major, 0, 0),
    }
}

fn server(product: Product, major: u32) -> ServerVersion {
    ServerVersion {
        product,
        version: version(major, 2, 0),
    }
}

#[test]
fn parses_every_version_line_the_tools_print() {
    let cases = [
        (
            "pg_dump (PostgreSQL) 17.2\n",
            Product::Postgres,
            version(17, 2, 0),
        ),
        (
            "pg_dump (PostgreSQL) 14.17 (Homebrew)",
            Product::Postgres,
            version(14, 17, 0),
        ),
        (
            "psql (PostgreSQL) 18beta1",
            Product::Postgres,
            version(18, 0, 0),
        ),
        (
            "mysqldump  Ver 8.4.3 for macos14.7 on arm64 (Homebrew)",
            Product::Mysql,
            version(8, 4, 3),
        ),
        (
            "mysqldump  Ver 10.13 Distrib 5.7.44, for Linux (x86_64)",
            Product::Mysql,
            version(5, 7, 44),
        ),
        (
            "mysqldump  Ver 10.19 Distrib 10.11.6-MariaDB, for debian-linux-gnu (x86_64)",
            Product::Mariadb,
            version(10, 11, 6),
        ),
        (
            "mariadb-dump from 11.4.2-MariaDB, client 10.19 for Linux (aarch64)",
            Product::Mariadb,
            version(11, 4, 2),
        ),
        (
            "mysql  Ver 8.0.36 for Linux on x86_64 (MySQL Community Server - GPL)",
            Product::Mysql,
            version(8, 0, 36),
        ),
        (
            "mariadb from 11.4.2-MariaDB, client 15.2 for debian-linux-gnu",
            Product::Mariadb,
            version(11, 4, 2),
        ),
    ];
    for (line, product, expected) in cases {
        assert_eq!(
            parse_tool_version(line),
            Some(ToolVersion {
                product,
                version: expected
            }),
            "{line}"
        );
    }
}

#[test]
fn rejects_output_without_a_version() {
    assert_eq!(parse_tool_version(""), None);
    assert_eq!(parse_tool_version("usage: frobnicate [options]"), None);
    assert_eq!(parse_tool_version("pg_dump (PostgreSQL) devel"), None);
}

#[test]
fn server_versions_parse_from_their_leading_number() {
    assert_eq!(Version::parse("17.2"), Some(version(17, 2, 0)));
    assert_eq!(
        Version::parse("11.4.2-MariaDB-log"),
        Some(version(11, 4, 2))
    );
    assert_eq!(Version::parse("16"), Some(version(16, 0, 0)));
    assert_eq!(Version::parse("beta"), None);
    assert_eq!(version(8, 4, 3).to_string(), "8.4.3");
    assert_eq!(version(17, 2, 0).to_string(), "17.2");
}

#[test]
fn pg_dump_older_than_the_server_is_flagged() {
    let warning = compatibility(
        ToolKind::PgDump,
        tool(Product::Postgres, 14),
        Some(server(Product::Postgres, 17)),
    );
    let warning = warning.unwrap_or_default();
    assert!(warning.contains("pg_dump 14.0 is older than the PostgreSQL 17.2 server"));
    assert!(warning.contains("refuses to back up newer servers"));
    assert!(warning.contains("version 17 or newer"));
}

#[test]
fn equal_or_newer_clients_are_fine() {
    for client in [17, 18] {
        let pg = tool(Product::Postgres, client);
        assert_eq!(
            compatibility(ToolKind::PgDump, pg, Some(server(Product::Postgres, 17))),
            None
        );
    }
    assert_eq!(
        compatibility(ToolKind::PgDump, tool(Product::Postgres, 14), None),
        None
    );
}

#[test]
fn restore_tools_explain_their_own_limits() {
    let old = tool(Product::Postgres, 15);
    let restore = compatibility(
        ToolKind::PgRestore,
        old,
        Some(server(Product::Postgres, 16)),
    );
    assert!(restore.is_some_and(|text| text.contains("archives made by a newer pg_dump")));
    let mysql = compatibility(
        ToolKind::Mysql,
        tool(Product::Mysql, 5),
        Some(server(Product::Mysql, 8)),
    );
    assert!(mysql.is_some_and(|text| text.contains("older than the MySQL 8.2 server")));
}

#[test]
fn foreign_clients_are_flagged() {
    let mariadb = compatibility(
        ToolKind::MysqlDump,
        tool(Product::Mariadb, 11),
        Some(server(Product::Mysql, 8)),
    );
    assert!(mariadb.is_some_and(|text| text.contains("is the MariaDB client")));
    let wrong = compatibility(
        ToolKind::PgDump,
        tool(Product::Postgres, 17),
        Some(server(Product::Mysql, 8)),
    );
    assert!(wrong.is_some_and(|text| text.contains("not for MySQL")));
}
