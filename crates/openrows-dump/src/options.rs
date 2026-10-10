// SPDX-License-Identifier: Apache-2.0
//! Backup and restore options, as the sheets offer them.
use crate::error::DumpError;

/// `pg_dump` output formats; `MySQL` dumps are always plain SQL.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PgFormat {
    /// A `.sql` script (`-Fp`).
    Plain,
    /// The compressed custom archive (`-Fc`), restored with `pg_restore`.
    #[default]
    Custom,
    /// One file per table in a folder (`-Fd`).
    Directory,
    /// A tar archive (`-Ft`).
    Tar,
}

impl PgFormat {
    /// The `--format` letter.
    pub(crate) fn letter(self) -> &'static str {
        match self {
            Self::Plain => "p",
            Self::Custom => "c",
            Self::Directory => "d",
            Self::Tar => "t",
        }
    }

    /// The usual file extension (none for a folder).
    #[must_use]
    pub fn extension(self) -> Option<&'static str> {
        match self {
            Self::Plain => Some("sql"),
            Self::Custom => Some("dump"),
            Self::Directory => None,
            Self::Tar => Some("tar"),
        }
    }
}

/// What a backup holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Content {
    /// Definitions and data.
    #[default]
    All,
    /// Definitions only.
    SchemaOnly,
    /// Data only.
    DataOnly,
}

/// A table chosen for a backup.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TableRef {
    /// Its schema (the open database for `MySQL`).
    pub schema: String,
    /// Its name.
    pub name: String,
}

/// Backup options; options another engine lacks are ignored there.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent switches, one per tool flag"
)]
pub struct BackupOptions {
    /// `PostgreSQL` output format.
    pub format: PgFormat,
    /// Definitions, data or both.
    pub content: Content,
    /// Only these schemas (`PostgreSQL`); empty for all.
    pub schemas: Vec<String>,
    /// Only these tables; empty for all.
    pub tables: Vec<TableRef>,
    /// Tables whose definitions are included but data is omitted (`PostgreSQL`).
    pub exclude_table_data: Vec<TableRef>,
    /// Leave out ownership (`--no-owner`).
    pub no_owner: bool,
    /// Leave out grants (`--no-privileges`).
    pub no_privileges: bool,
    /// Compression level 0–9 (`PostgreSQL` plain, custom and directory).
    pub compression: Option<u8>,
    /// One consistent snapshot without locking (`MySQL` `--single-transaction`).
    pub single_transaction: bool,
    /// Stored procedures and functions (`MySQL` `--routines`).
    pub routines: bool,
    /// Triggers (`MySQL`; on unless turned off).
    pub triggers: bool,
    /// Scheduled events (`MySQL` `--events`).
    pub events: bool,
}

impl Default for BackupOptions {
    fn default() -> Self {
        Self {
            format: PgFormat::Custom,
            content: Content::All,
            schemas: Vec::new(),
            tables: Vec::new(),
            exclude_table_data: Vec::new(),
            no_owner: false,
            no_privileges: false,
            compression: None,
            single_transaction: true,
            routines: true,
            triggers: true,
            events: false,
        }
    }
}

impl BackupOptions {
    /// Reject combinations `pg_dump` would refuse.
    ///
    /// # Errors
    /// `Invalid` naming the problem.
    pub fn validate(&self) -> Result<(), DumpError> {
        if let Some(level) = self.compression {
            if level > 9 {
                return Err(DumpError::invalid("Compression must be between 0 and 9"));
            }
            if self.format == PgFormat::Tar {
                return Err(DumpError::invalid("Tar archives cannot be compressed"));
            }
        }
        let names = self.schemas.iter().chain(
            self.tables
                .iter()
                .chain(&self.exclude_table_data)
                .flat_map(|table| [&table.schema, &table.name]),
        );
        if names
            .into_iter()
            .any(|name| name.is_empty() || name.contains('\0'))
        {
            return Err(DumpError::invalid(
                "Schema and table names must be nonempty and contain no NUL",
            ));
        }
        Ok(())
    }
}

/// Whether `pg_restore` drops objects before recreating them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Clean {
    /// Keep existing objects.
    #[default]
    Keep,
    /// `--clean`: drop them first.
    Drop,
    /// `--clean --if-exists`: drop them first, if they exist.
    DropIfExists,
}

/// Restore options; `pg_restore` options are ignored for SQL scripts.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct RestoreOptions {
    /// Restore only these `PostgreSQL` schemas; empty restores all.
    pub schemas: Vec<String>,
    /// Drop objects before recreating them.
    pub clean: Clean,
    /// Do not restore ownership (`--no-owner`).
    pub no_owner: bool,
    /// Do not restore grants (`--no-privileges`).
    pub no_privileges: bool,
    /// Parallel jobs (`-j`); 0 and 1 run serially.
    pub jobs: u16,
    /// Stop at the first error instead of carrying on.
    pub stop_on_error: bool,
    /// Archive entries (dump ids from the list) to restore; `None` for all.
    pub selected: Option<Vec<u32>>,
}

impl RestoreOptions {
    /// Reject combinations `pg_restore` would refuse.
    ///
    /// # Errors
    /// `Invalid` naming the problem.
    pub fn validate(&self, tar: bool) -> Result<(), DumpError> {
        if self
            .schemas
            .iter()
            .any(|name| name.is_empty() || name.contains('\0'))
        {
            return Err(DumpError::invalid(
                "Restore schemas must be nonempty and contain no NUL",
            ));
        }
        if self.jobs > 64 {
            return Err(DumpError::invalid("Use at most 64 parallel jobs"));
        }
        if tar && self.jobs > 1 {
            return Err(DumpError::invalid(
                "Tar archives cannot be restored with parallel jobs",
            ));
        }
        if self.selected.as_ref().is_some_and(Vec::is_empty) {
            return Err(DumpError::invalid("Select at least one item to restore"));
        }
        Ok(())
    }
}
