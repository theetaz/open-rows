// SPDX-License-Identifier: Apache-2.0
//! Public pgpass, libpq service and `MySQL` option-file imports (FR-CONN-09).
mod ini;
mod pgpass;
mod sections;
use super::{
    ProfileError,
    exchange::{ExchangeError, ImportBatch},
};
pub use pgpass::parse_pgpass;
pub use sections::{parse_my_cnf, parse_pg_service};
use std::{
    io::Read as _,
    path::{Path, PathBuf},
};
use zeroize::Zeroizing;

/// A supported public text format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolFormat {
    /// Five-field libpq password file.
    Pgpass,
    /// libpq service INI sections.
    PgService,
    /// `MySQL` client option groups.
    MyCnf,
}
/// Parse a source using its public grammar, retaining passwords separately.
/// # Errors
/// Invalid port, TLS mode or concrete profile settings.
pub fn parse(format: ToolFormat, text: &str) -> Result<ImportBatch, ExchangeError> {
    match format {
        ToolFormat::Pgpass => parse_pgpass(text),
        ToolFormat::PgService => parse_pg_service(text),
        ToolFormat::MyCnf => parse_my_cnf(text),
    }
}
/// Read a bounded public file; absence is a preview note, not an error.
/// # Errors
/// I/O, oversized source or invalid file contents.
pub fn read(format: ToolFormat, path: &Path) -> Result<ImportBatch, ExchangeError> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ImportBatch {
                warnings: vec![format!("{}: not found", path.display())],
                ..ImportBatch::default()
            });
        }
        Err(source) => {
            return Err(ProfileError::Io {
                path: path.into(),
                source,
            }
            .into());
        }
    };
    let mut text = Zeroizing::new(String::new());
    file.take(4 * 1024 * 1024 + 1)
        .read_to_string(&mut text)
        .map_err(|source| ProfileError::Io {
            path: path.into(),
            source,
        })?;
    if text.len() > 4 * 1024 * 1024 {
        return Err(ExchangeError::Invalid("import file exceeds 4 MiB".into()));
    }
    parse(format, &text)
}
/// Public default locations, with libpq environment overrides respected.
#[must_use]
pub fn locations(format: ToolFormat, home: &Path) -> Vec<PathBuf> {
    match format {
        ToolFormat::Pgpass => {
            vec![std::env::var_os("PGPASSFILE").map_or_else(|| home.join(".pgpass"), PathBuf::from)]
        }
        ToolFormat::PgService => {
            let mut paths = vec![
                std::env::var_os("PGSERVICEFILE")
                    .map_or_else(|| home.join(".pg_service.conf"), PathBuf::from),
            ];
            if let Some(system) = std::env::var_os("PGSYSCONFDIR") {
                paths.push(PathBuf::from(system).join("pg_service.conf"));
            }
            paths
        }
        ToolFormat::MyCnf => vec![home.join(".my.cnf")],
    }
}
