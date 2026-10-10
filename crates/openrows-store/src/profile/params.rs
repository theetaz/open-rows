// SPDX-License-Identifier: Apache-2.0
//! The engine a profile connects to and its parameters (everything except
//! secrets): host-based engines use [`ServerParams`], `SQLite` uses
//! [`FileParams`].

use std::{collections::BTreeMap, path::PathBuf};

use serde::{Deserialize, Serialize};

use super::AttachedFile;

/// Database engine of a profile, stored as `"driver"`. Profiles written
/// before D-12 always say `"postgres"`, which keeps its meaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Driver {
    /// Embedded analytical database, always isolated in the driver host.
    Duckdb,
    /// `PostgreSQL` and wire-compatible servers.
    #[default]
    Postgres,
    /// `MySQL` (and `MySQL`-wire servers such as `TiDB`, `PlanetScale`).
    Mysql,
    /// `MariaDB`.
    Mariadb,
    /// SQL Server and Azure SQL.
    Sqlserver,
    /// `ClickHouse` over HTTP(S).
    Clickhouse,
    /// A local `SQLite` 3 file.
    Sqlite,
    /// Remote libSQL/Turso through Hrana 3 HTTP.
    Libsql,
    /// SQLite-compatible local libSQL file.
    #[serde(rename = "libsql-file")]
    LibsqlFile,
}

impl Driver {
    /// Every engine, in picker order.
    pub const ALL: [Self; 9] = [
        Self::Postgres,
        Self::Mysql,
        Self::Mariadb,
        Self::Sqlserver,
        Self::Clickhouse,
        Self::Sqlite,
        Self::Libsql,
        Self::LibsqlFile,
        Self::Duckdb,
    ];

    /// Display name, e.g. `MariaDB`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Duckdb => "DuckDB",
            Self::Postgres => "PostgreSQL",
            Self::Mysql => "MySQL",
            Self::Mariadb => "MariaDB",
            Self::Sqlserver => "SQL Server",
            Self::Clickhouse => "ClickHouse",
            Self::Sqlite => "SQLite",
            Self::Libsql => "libSQL / Turso (remote)",
            Self::LibsqlFile => "libSQL file",
        }
    }

    /// Default TCP port; `None` for file engines.
    #[must_use]
    pub fn default_port(self) -> Option<u16> {
        match self {
            Self::Postgres => Some(5432),
            Self::Mysql | Self::Mariadb => Some(3306),
            Self::Sqlserver => Some(1433),
            Self::Clickhouse => Some(8123),
            Self::Sqlite | Self::LibsqlFile | Self::Duckdb => None,
            Self::Libsql => Some(443),
        }
    }

    /// True for engines that open a local file instead of a server.
    #[must_use]
    pub fn is_file(self) -> bool {
        matches!(self, Self::Sqlite | Self::LibsqlFile | Self::Duckdb)
    }
}

/// Host-based connection parameters (`PostgreSQL`, `MySQL`, `MariaDB`, SQL Server).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerParams {
    /// Host name, IP literal (IPv6 without brackets) or socket directory.
    pub host: String,
    /// TCP port.
    pub port: u16,
    /// Database name; the engine's home database is used when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database: Option<String>,
    /// Login user.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    /// Name reported to the server (`application_name`, `program_name`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application_name: Option<String>,
    /// Other driver parameters (e.g. `connect_timeout`), passed through.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub options: BTreeMap<String, String>,
}

impl ServerParams {
    /// `localhost` on `port`, nothing else set.
    #[must_use]
    pub fn local(port: u16) -> Self {
        Self {
            host: "localhost".to_owned(),
            port,
            database: None,
            user: None,
            application_name: None,
            options: BTreeMap::new(),
        }
    }
}

impl Default for ServerParams {
    /// The `PostgreSQL` default, `localhost:5432`.
    fn default() -> Self {
        Self::local(5432)
    }
}

/// A local database file or an in-memory database (`SQLite`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileParams {
    /// Absolute path of the database file; empty for an in-memory database.
    #[serde(default)]
    pub path: PathBuf,
    /// An in-memory database shared by the profile's connections while the
    /// app runs; `path`, `read_only` and `create` do not apply.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub memory: bool,
    /// Open the file read-only: the engine itself refuses every write.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub read_only: bool,
    /// Create the file on connect when it does not exist yet (chosen with
    /// "New file…").
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub create: bool,
    /// Other database files attached to every connection, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attached: Vec<AttachedFile>,
}

impl FileParams {
    /// An in-memory database with nothing attached.
    #[must_use]
    pub fn memory() -> Self {
        Self {
            memory: true,
            ..Self::default()
        }
    }
}
