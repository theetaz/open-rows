// SPDX-License-Identifier: Apache-2.0
//! Webview DTOs for a profile's engine and, for file engines, its database
//! file (D-12).
use std::path::PathBuf;

use openrows_store::profile::{AttachedFile, Driver, FileParams};
use serde::{Deserialize, Serialize};
use specta::Type;

/// Database engine of a profile.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum EngineDto {
    Duckdb,
    #[default]
    Postgres,
    Mysql,
    Mariadb,
    Sqlserver,
    Clickhouse,
    Sqlite,
    Libsql,
    Mongodb,
    #[serde(rename = "libsql-file")]
    LibsqlFile,
}

impl From<Driver> for EngineDto {
    fn from(driver: Driver) -> Self {
        match driver {
            Driver::Duckdb => Self::Duckdb,
            Driver::Postgres => Self::Postgres,
            Driver::Mysql => Self::Mysql,
            Driver::Mariadb => Self::Mariadb,
            Driver::Sqlserver => Self::Sqlserver,
            Driver::Clickhouse => Self::Clickhouse,
            Driver::Sqlite => Self::Sqlite,
            Driver::Libsql => Self::Libsql,
            Driver::Mongodb => Self::Mongodb,
            Driver::LibsqlFile => Self::LibsqlFile,
        }
    }
}

impl From<EngineDto> for Driver {
    fn from(engine: EngineDto) -> Self {
        match engine {
            EngineDto::Duckdb => Self::Duckdb,
            EngineDto::Postgres => Self::Postgres,
            EngineDto::Mysql => Self::Mysql,
            EngineDto::Mariadb => Self::Mariadb,
            EngineDto::Sqlserver => Self::Sqlserver,
            EngineDto::Clickhouse => Self::Clickhouse,
            EngineDto::Sqlite => Self::Sqlite,
            EngineDto::Libsql => Self::Libsql,
            EngineDto::Mongodb => Self::Mongodb,
            EngineDto::LibsqlFile => Self::LibsqlFile,
        }
    }
}

/// The database file of a file engine, in the form and in saved profiles.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FileDto {
    /// Absolute path; empty for an in-memory database.
    pub(crate) path: String,
    /// An in-memory database shared by the connection's sessions while the
    /// app runs (absent: a file).
    #[serde(default)]
    pub(crate) memory: bool,
    /// Open read-only (the engine refuses writes).
    pub(crate) read_only: bool,
    /// Create the file on connect when it is missing ("New file…").
    pub(crate) create: bool,
    /// Attached database files. Read-only for the form: saving keeps the
    /// stored list, which only `connection_attach_database` and
    /// `connection_detach_database` change.
    #[serde(default)]
    pub(crate) attached: Vec<AttachedFileDto>,
}

/// A database file attached under a schema name.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AttachedFileDto {
    /// Schema name.
    pub(crate) alias: String,
    /// Absolute path.
    pub(crate) path: String,
}

impl From<&AttachedFile> for AttachedFileDto {
    fn from(file: &AttachedFile) -> Self {
        Self {
            alias: file.alias.clone(),
            path: file.path.display().to_string(),
        }
    }
}

impl From<&FileParams> for FileDto {
    fn from(file: &FileParams) -> Self {
        Self {
            path: file.path.display().to_string(),
            memory: file.memory,
            read_only: file.read_only,
            create: file.create,
            attached: file.attached.iter().map(AttachedFileDto::from).collect(),
        }
    }
}

impl FileDto {
    /// The form's file settings with `attached` kept from the saved profile.
    /// In-memory databases have no path and are never read-only or created.
    pub(crate) fn to_params(&self, attached: Vec<AttachedFile>) -> FileParams {
        if self.memory {
            return FileParams {
                attached,
                ..FileParams::memory()
            };
        }
        FileParams {
            path: PathBuf::from(self.path.trim()),
            memory: false,
            read_only: self.read_only,
            create: self.create,
            attached,
        }
    }
}
