// SPDX-License-Identifier: Apache-2.0
//! Webview DTOs for a profile's engine and, for file engines, its database
//! file (D-12).
use std::path::PathBuf;

use openrows_store::profile::{Driver, FileParams};
use serde::{Deserialize, Serialize};
use specta::Type;

/// Database engine of a profile.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum EngineDto {
    #[default]
    Postgres,
    Mysql,
    Mariadb,
    Sqlite,
}

impl From<Driver> for EngineDto {
    fn from(driver: Driver) -> Self {
        match driver {
            Driver::Postgres => Self::Postgres,
            Driver::Mysql => Self::Mysql,
            Driver::Mariadb => Self::Mariadb,
            Driver::Sqlite => Self::Sqlite,
        }
    }
}

impl From<EngineDto> for Driver {
    fn from(engine: EngineDto) -> Self {
        match engine {
            EngineDto::Postgres => Self::Postgres,
            EngineDto::Mysql => Self::Mysql,
            EngineDto::Mariadb => Self::Mariadb,
            EngineDto::Sqlite => Self::Sqlite,
        }
    }
}

/// The database file of a file engine, in the form and in saved profiles.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FileDto {
    /// Absolute path.
    pub(crate) path: String,
    /// Open read-only (the engine refuses writes).
    pub(crate) read_only: bool,
    /// Create the file on connect when it is missing ("New file…").
    pub(crate) create: bool,
}

impl From<&FileParams> for FileDto {
    fn from(file: &FileParams) -> Self {
        Self {
            path: file.path.display().to_string(),
            read_only: file.read_only,
            create: file.create,
        }
    }
}

impl From<&FileDto> for FileParams {
    fn from(file: &FileDto) -> Self {
        Self {
            path: PathBuf::from(file.path.trim()),
            read_only: file.read_only,
            create: file.create,
        }
    }
}
