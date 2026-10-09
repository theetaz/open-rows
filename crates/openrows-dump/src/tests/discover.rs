// SPDX-License-Identifier: Apache-2.0
#![cfg(unix)]
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

use super::*;
use crate::tools::{expand, search_dirs};
use crate::version::{Product, Version};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// A fake tool that prints `version` for `--version`.
fn fake(dir: &Path, name: &str, version: &str) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\necho '{version}'\n"))?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
    Ok(path)
}

fn found(kind: ToolKind, product: Product, major: u32) -> FoundTool {
    FoundTool {
        kind,
        path: PathBuf::from(format!("/opt/{major}/{}", kind.label())),
        version: ToolVersion {
            product,
            version: Version {
                major,
                minor: 1,
                patch: 0,
            },
        },
    }
}

fn server(product: Product, major: u32) -> ServerVersion {
    ServerVersion {
        product,
        version: Version {
            major,
            minor: 0,
            patch: 0,
        },
    }
}

#[test]
fn finds_and_probes_tools_in_search_order() -> TestResult {
    let root = tempfile::tempdir()?;
    let (brew, libpq) = (root.path().join("brew"), root.path().join("libpq"));
    fake(&brew, "pg_dump", "pg_dump (PostgreSQL) 14.17 (Homebrew)")?;
    fake(&libpq, "pg_dump", "pg_dump (PostgreSQL) 17.4")?;
    // Not executable: skipped.
    std::fs::write(root.path().join("pg_dump"), "x")?;
    let dirs = vec![root.path().to_path_buf(), brew.clone(), libpq.clone()];
    let tools = discover(ToolKind::PgDump, &dirs);
    let versions: Vec<_> = tools
        .iter()
        .map(|tool| tool.version.version.major)
        .collect();
    assert_eq!(versions, [14, 17]);
    assert_eq!(tools[0].path, brew.join("pg_dump"));
    Ok(())
}

#[test]
fn symlinked_copies_are_listed_once_and_mariadb_names_are_found() -> TestResult {
    let root = tempfile::tempdir()?;
    let cellar = root.path().join("cellar");
    let real = fake(
        &cellar,
        "mariadb-dump",
        "mariadb-dump from 11.4.2-MariaDB, client 10.19",
    )?;
    let bin = root.path().join("bin");
    std::fs::create_dir_all(&bin)?;
    std::os::unix::fs::symlink(&real, bin.join("mariadb-dump"))?;
    let paths = candidates(ToolKind::MysqlDump, &[bin.clone(), cellar]);
    assert_eq!(paths, [bin.join("mariadb-dump")]);
    Ok(())
}

#[test]
fn probing_rejects_other_programs_and_wrong_families() -> TestResult {
    let root = tempfile::tempdir()?;
    let other = fake(root.path(), "other", "frobnicate 1.0")?;
    assert!(matches!(
        probe(ToolKind::PgDump, &other),
        Err(DumpError::WrongTool { .. })
    ));
    let mysql = fake(root.path(), "mysqldump", "mysqldump  Ver 8.4.3 for macos")?;
    assert!(probe(ToolKind::PgDump, &mysql).is_err());
    assert_eq!(
        probe(ToolKind::MysqlDump, &mysql)?.version.product,
        Product::Mysql
    );
    assert!(matches!(
        probe(ToolKind::Psql, &root.path().join("missing")),
        Err(DumpError::NotExecutable { .. })
    ));
    Ok(())
}

#[test]
fn chooses_the_closest_release_at_least_as_new_as_the_server() {
    let tools = [14, 18, 17, 16].map(|major| found(ToolKind::PgDump, Product::Postgres, major));
    let major = |server| choose(&tools, server).map(|tool| tool.version.version.major);
    assert_eq!(major(Some(server(Product::Postgres, 17))), Some(17));
    assert_eq!(major(Some(server(Product::Postgres, 15))), Some(16));
    // Nothing new enough: the newest, which comes closest.
    assert_eq!(major(Some(server(Product::Postgres, 19))), Some(18));
    assert_eq!(major(None), Some(18));
    assert_eq!(choose(&[], None), None);
}

#[test]
fn prefers_the_servers_own_product_and_earlier_paths_on_ties() {
    let mysql = found(ToolKind::MysqlDump, Product::Mysql, 8);
    let mariadb = found(ToolKind::MysqlDump, Product::Mariadb, 11);
    let tools = [mysql.clone(), mariadb.clone()];
    assert_eq!(
        choose(&tools, Some(server(Product::Mariadb, 11))),
        Some(&mariadb)
    );
    assert_eq!(
        choose(&tools, Some(server(Product::Mysql, 8))),
        Some(&mysql)
    );
    let mut twin = mysql.clone();
    twin.path = PathBuf::from("/later/mysqldump");
    assert_eq!(choose(&[mysql.clone(), twin], None), Some(&mysql));
}

#[test]
fn search_dirs_start_with_path_and_expand_wildcards() -> TestResult {
    let root = tempfile::tempdir()?;
    for version in ["postgresql@15", "postgresql@14", "mysql-client"] {
        std::fs::create_dir_all(root.path().join(version).join("bin"))?;
    }
    let pattern = root.path().join("postgresql@*").join("bin");
    assert_eq!(
        expand(&pattern),
        [
            root.path().join("postgresql@14/bin"),
            root.path().join("postgresql@15/bin")
        ]
    );
    let path = std::env::join_paths([root.path(), Path::new("relative"), root.path()])?;
    let dirs = search_dirs(Some(&path));
    assert_eq!(dirs.first().map(PathBuf::as_path), Some(root.path()));
    assert_eq!(dirs.iter().filter(|dir| *dir == root.path()).count(), 1);
    assert!(!dirs.iter().any(|dir| dir == Path::new("relative")));
    assert!(dirs.iter().any(|dir| dir == Path::new("/usr/bin")));
    Ok(())
}
