// SPDX-License-Identifier: Apache-2.0
//! The client tools `OpenRows` drives and where they are usually installed.
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

/// Which engines a tool works with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// `PostgreSQL` and compatible servers.
    Postgres,
    /// `MySQL` and `MariaDB`.
    Mysql,
}

/// One client tool.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ToolKind {
    /// `pg_dump`: backups of a `PostgreSQL` database.
    PgDump,
    /// `pg_restore`: restores and lists `pg_dump` archives.
    PgRestore,
    /// `psql`: runs plain SQL dumps and creates databases.
    Psql,
    /// `mysqldump` or `mariadb-dump`.
    MysqlDump,
    /// `mysql` or `mariadb`: runs SQL dumps and creates databases.
    Mysql,
}

impl ToolKind {
    /// Every tool, in Settings order.
    pub const ALL: [Self; 5] = [
        Self::PgDump,
        Self::PgRestore,
        Self::Psql,
        Self::MysqlDump,
        Self::Mysql,
    ];

    /// Executable names, preferred first.
    #[must_use]
    pub fn names(self) -> &'static [&'static str] {
        match self {
            Self::PgDump => &["pg_dump"],
            Self::PgRestore => &["pg_restore"],
            Self::Psql => &["psql"],
            Self::MysqlDump => &["mysqldump", "mariadb-dump"],
            Self::Mysql => &["mysql", "mariadb"],
        }
    }

    /// The name shown to users.
    #[must_use]
    pub fn label(self) -> &'static str {
        self.names()[0]
    }

    /// The engines it works with.
    #[must_use]
    pub fn family(self) -> Family {
        match self {
            Self::PgDump | Self::PgRestore | Self::Psql => Family::Postgres,
            Self::MysqlDump | Self::Mysql => Family::Mysql,
        }
    }
}

/// Install locations searched after `PATH`: apps started from the Finder
/// get a minimal `PATH`, so Homebrew, Postgres.app and the official
/// installers are listed explicitly. A `*` matches one directory level.
const COMMON_DIRS: &[&str] = &[
    "/opt/homebrew/bin",
    "/usr/local/bin",
    "/opt/homebrew/opt/libpq/bin",
    "/usr/local/opt/libpq/bin",
    "/opt/homebrew/opt/postgresql@*/bin",
    "/usr/local/opt/postgresql@*/bin",
    "/Applications/Postgres.app/Contents/Versions/*/bin",
    "/opt/homebrew/opt/mysql-client/bin",
    "/usr/local/opt/mysql-client/bin",
    "/opt/homebrew/opt/mysql-client@*/bin",
    "/usr/local/opt/mysql-client@*/bin",
    "/opt/homebrew/opt/mysql/bin",
    "/opt/homebrew/opt/mysql@*/bin",
    "/opt/homebrew/opt/mariadb/bin",
    "/opt/homebrew/opt/mariadb@*/bin",
    "/usr/local/mysql/bin",
    "/usr/lib/postgresql/*/bin",
    "/usr/pgsql-*/bin",
    "/usr/bin",
];

/// Directories to search: every `PATH` entry, then [`COMMON_DIRS`].
#[must_use]
pub fn search_dirs(path_variable: Option<&OsStr>) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = path_variable
        .map(|value| std::env::split_paths(value).collect())
        .unwrap_or_default();
    for pattern in COMMON_DIRS {
        dirs.extend(expand(Path::new(pattern)));
    }
    let mut seen = std::collections::HashSet::new();
    dirs.retain(|dir| dir.is_absolute() && seen.insert(dir.clone()));
    dirs
}

/// `pattern` with each component holding one `*` replaced by every
/// matching entry, sorted; a pattern without `*` is returned as is.
pub(crate) fn expand(pattern: &Path) -> Vec<PathBuf> {
    let mut found = vec![PathBuf::new()];
    for component in pattern.components() {
        let part = component.as_os_str().to_string_lossy();
        let Some((prefix, suffix)) = part.split_once('*') else {
            for path in &mut found {
                path.push(component);
            }
            continue;
        };
        found = found
            .iter()
            .flat_map(|base| matching(base, prefix, suffix))
            .collect();
    }
    found
}

fn matching(base: &Path, prefix: &str, suffix: &str) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(base) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| {
            name.len() >= prefix.len() + suffix.len()
                && name.starts_with(prefix)
                && name.ends_with(suffix)
        })
        .collect();
    names.sort();
    names.into_iter().map(|name| base.join(name)).collect()
}

/// Whether `path` is a regular file the user may execute.
pub(crate) fn is_executable(path: &Path) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        meta.is_file() && meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        meta.is_file()
    }
}
