// SPDX-License-Identifier: Apache-2.0
//! Backup and restore through the client tools the user installed
//! (FR-IO-04): `pg_dump`, `pg_restore` and `psql` for `PostgreSQL`,
//! `mysqldump`/`mariadb-dump` and `mysql`/`mariadb` for `MySQL` and
//! `MariaDB`, plus a native `SQLite` copy.
//!
//! [`search_dirs`] and [`discover`] find every installed copy on `PATH` and
//! in the usual install folders, [`probe`] asks each for its version, and
//! [`choose`] picks the one matching the server; [`compatibility`] explains
//! why a client may not work (`pg_dump` refuses newer servers).
//!
//! Commands are argument vectors spawned directly, never through a shell
//! ([`ToolCommand`]); [`ToolCommand::display`] shows them before they run.
//! Passwords never appear on a command line or in the environment: the
//! tools read them from a 0600 [`PrivateFile`] (`PGPASSFILE`, `MySQL`
//! `--defaults-extra-file`) that is deleted afterwards. [`run`] streams the
//! tool's stderr, reports progress and cancels the whole process group.
//! [`restore_gate`] and [`admit`] keep restores, which write, behind
//! read-only mode and confirmation.
#![forbid(unsafe_code)]
mod archive;
mod command;
mod conn;
mod discover;
mod error;
mod mysql;
mod options;
mod pg;
mod run;
mod safety;
mod secret_file;
mod sqlite;
mod tools;
mod version;

pub use archive::{ArchiveEntry, DumpKind, detect, parse_list, use_list};
pub use command::{PASSWORD_FILE_MARK, ProgressKind, ToolCommand};
pub use conn::{Address, ConnectionSpec, TlsMode, TlsSpec};
pub use discover::{FoundTool, candidates, choose, discover, probe};
pub use error::DumpError;
pub use mysql::{mysql_create_database, mysql_script, mysqldump};
pub use options::{BackupOptions, Clean, Content, PgFormat, RestoreOptions, TableRef};
pub use pg::{conninfo, pg_dump, pg_list, pg_restore, psql_create_database, psql_script};
pub use run::{Canceller, RunEvent, RunOutcome, capture, run};
pub use safety::{Confirmation, RestoreGate, RestoreRisk, admit, confirmation_token, restore_gate};
pub use secret_file::PrivateFile;
pub use sqlite::{sqlite_backup, sqlite_restore};
pub use tools::{Family, ToolKind, search_dirs};
pub use version::{
    Product, ServerVersion, ToolVersion, Version, compatibility, parse_tool_version,
};
