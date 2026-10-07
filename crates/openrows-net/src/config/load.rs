// SPDX-License-Identifier: Apache-2.0
//! `Include` expansion into a directive tree, behind an injectable file source.

use std::{
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
};

use super::{
    ConfigError,
    parse::{Directive, parse_lines},
    pattern::file_glob_match,
};

/// Same nesting limit as OpenSSH (`READCONF_MAX_DEPTH`); also stops cycles.
pub(crate) const MAX_INCLUDE_DEPTH: usize = 16;

/// Where configuration text comes from; tests supply an in-memory map.
pub trait ConfigSource {
    /// Read a whole file.
    ///
    /// # Errors
    /// Any I/O error; `NotFound` for included files is ignored by the loader.
    fn read(&self, path: &Path) -> io::Result<String>;

    /// List the entries of a directory, for `Include` globs.
    ///
    /// # Errors
    /// Any I/O error; `NotFound` is treated as an empty directory.
    fn list(&self, dir: &Path) -> io::Result<Vec<PathBuf>>;
}

/// The real file system.
#[derive(Clone, Copy, Debug, Default)]
pub struct FileSystem;

impl ConfigSource for FileSystem {
    fn read(&self, path: &Path) -> io::Result<String> {
        fs::read_to_string(path)
    }

    fn list(&self, dir: &Path) -> io::Result<Vec<PathBuf>> {
        fs::read_dir(dir)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect()
    }
}

/// A directive, or the contents of an `Include` evaluated in place.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Node {
    Directive(Directive),
    Include(Vec<Node>),
}

pub(crate) struct Loader<'a> {
    pub(crate) home: &'a Path,
    pub(crate) source: &'a dyn ConfigSource,
}

impl Loader<'_> {
    pub(crate) fn parse_text(
        &self,
        text: &str,
        path: &Path,
        depth: usize,
    ) -> Result<Vec<Node>, ConfigError> {
        let lines = parse_lines(text).map_err(|error| ConfigError::Syntax {
            path: path.to_path_buf(),
            line: error.number,
            message: error.message,
        })?;
        lines
            .into_iter()
            .map(|line| match line.directive {
                Directive::Include(patterns) => {
                    self.include(&patterns, path, depth).map(Node::Include)
                }
                directive => Ok(Node::Directive(directive)),
            })
            .collect()
    }

    fn include(
        &self,
        patterns: &[String],
        from: &Path,
        depth: usize,
    ) -> Result<Vec<Node>, ConfigError> {
        if depth + 1 >= MAX_INCLUDE_DEPTH {
            return Err(ConfigError::IncludeDepth {
                path: from.to_path_buf(),
                max: MAX_INCLUDE_DEPTH,
            });
        }
        let mut paths = Vec::new();
        for pattern in patterns {
            paths.extend(self.expand(pattern)?);
        }
        let mut nodes = Vec::new();
        for path in paths {
            let Some(text) = read_optional(self.source.read(&path), &path)? else {
                continue;
            };
            nodes.extend(self.parse_text(&text, &path, depth + 1)?);
        }
        Ok(nodes)
    }

    fn expand(&self, pattern: &str) -> Result<Vec<PathBuf>, ConfigError> {
        let path = self.absolute(pattern);
        let name = path.file_name().and_then(OsStr::to_str).map(str::to_owned);
        let (Some(dir), Some(name)) = (path.parent(), name) else {
            return Ok(vec![path]);
        };
        if !name.contains(['*', '?']) {
            return Ok(vec![path]);
        }
        let mut matches = read_optional(self.source.list(dir), dir)?.unwrap_or_default();
        matches.retain(|candidate| {
            candidate
                .file_name()
                .and_then(OsStr::to_str)
                .is_some_and(|candidate| file_glob_match(&name, candidate))
        });
        matches.sort();
        Ok(matches)
    }

    /// `~` expands to home; relative user-config paths resolve under `~/.ssh`.
    fn absolute(&self, pattern: &str) -> PathBuf {
        let expanded = expand_tilde(pattern, self.home);
        if expanded.is_absolute() {
            expanded
        } else {
            self.home.join(".ssh").join(expanded)
        }
    }
}

/// Expand a leading `~` or `~/` to `home`.
pub(crate) fn expand_tilde(value: &str, home: &Path) -> PathBuf {
    match value.strip_prefix('~') {
        Some("") => home.to_path_buf(),
        Some(rest) if rest.starts_with('/') => home.join(rest.trim_start_matches('/')),
        _ => PathBuf::from(value),
    }
}

fn read_optional<T>(result: io::Result<T>, path: &Path) -> Result<Option<T>, ConfigError> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(ConfigError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}
