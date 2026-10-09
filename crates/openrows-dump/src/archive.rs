// SPDX-License-Identifier: Apache-2.0
//! What a backup file is, and the table of contents of `pg_dump` archives
//! (`pg_restore --list`) for restoring selected objects.
use std::fs::File;
use std::io::Read as _;
use std::path::Path;

use crate::error::DumpError;

/// The kind of backup a file or folder holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DumpKind {
    /// A SQL script, run with `psql` or `mysql`.
    PlainSql,
    /// A `pg_dump -Fc` archive.
    PgCustom,
    /// A `pg_dump -Ft` archive.
    PgTar,
    /// A `pg_dump -Fd` folder.
    PgDirectory,
}

impl DumpKind {
    /// Whether `pg_restore` reads it.
    #[must_use]
    pub fn is_archive(self) -> bool {
        self != Self::PlainSql
    }
}

/// Look at the first bytes (or the folder's `toc.dat`) to tell the kind.
///
/// # Errors
/// The path cannot be read, or is a folder that is not a `pg_dump` folder.
pub fn detect(path: &Path) -> Result<DumpKind, DumpError> {
    if path.is_dir() {
        return if path.join("toc.dat").is_file() {
            Ok(DumpKind::PgDirectory)
        } else {
            Err(DumpError::invalid(
                "The folder is not a pg_dump directory backup (it has no toc.dat)",
            ))
        };
    }
    let mut head = Vec::with_capacity(512);
    File::open(path)
        .and_then(|file| file.take(512).read_to_end(&mut head))
        .map_err(|error| DumpError::io("cannot read", path, error))?;
    if head.starts_with(b"PGDMP") {
        Ok(DumpKind::PgCustom)
    } else if head.get(257..262) == Some(b"ustar".as_slice()) {
        Ok(DumpKind::PgTar)
    } else {
        Ok(DumpKind::PlainSql)
    }
}

/// One entry of an archive's table of contents.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchiveEntry {
    /// The dump id `pg_restore --use-list` selects by.
    pub id: u32,
    /// What it is, e.g. `TABLE public accounts postgres`.
    pub label: String,
    /// Whether it is table data rather than a definition.
    pub data: bool,
    /// The list line as `pg_restore` printed it.
    line: String,
}

/// Entries of `pg_restore --list` output; comment lines are skipped.
/// A line reads `215; 1259 16386 TABLE public accounts postgres`.
#[must_use]
pub fn parse_list(text: &str) -> Vec<ArchiveEntry> {
    text.lines().filter_map(entry).collect()
}

fn entry(line: &str) -> Option<ArchiveEntry> {
    let trimmed = line.trim();
    if trimmed.starts_with(';') {
        return None;
    }
    let (id, rest) = trimmed.split_once(';')?;
    let id = id.trim().parse().ok()?;
    // Skip the catalog table oid and the object oid.
    let mut words = rest.split_whitespace();
    words.next()?;
    words.next()?;
    let label = words.collect::<Vec<_>>().join(" ");
    if label.is_empty() {
        return None;
    }
    Some(ArchiveEntry {
        id,
        data: label.starts_with("TABLE DATA") || label.starts_with("BLOBS"),
        label,
        line: trimmed.to_owned(),
    })
}

/// A `--use-list` file selecting the `selected` dump ids, in archive order.
#[must_use]
pub fn use_list(entries: &[ArchiveEntry], selected: &[u32]) -> String {
    let selected: std::collections::HashSet<u32> = selected.iter().copied().collect();
    let mut text = String::new();
    for entry in entries.iter().filter(|entry| selected.contains(&entry.id)) {
        text.push_str(&entry.line);
        text.push('\n');
    }
    text
}

#[cfg(test)]
#[path = "tests/archive.rs"]
mod tests;
