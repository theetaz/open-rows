// SPDX-License-Identifier: Apache-2.0
//! Request, reply and event types of the `dump_*` commands.
use openrows_dump::{
    BackupOptions, Clean, Content, DumpKind, PgFormat, Product, RestoreOptions, ServerVersion,
    TableRef, ToolKind, Version,
};
use serde::{Deserialize, Serialize};
use specta::Type;

/// How a database is backed up: the `PostgreSQL` or `MySQL` tools, or a
/// native `SQLite` copy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum DumpEngineDto {
    Postgres,
    Mysql,
    Sqlite,
}

/// A client tool.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum DumpToolKindDto {
    Aws,
    SessionManagerPlugin,
    Kubectl,
    CloudSqlProxy,
    AlloydbAuthProxy,
    Cloudflared,
    PgDump,
    PgRestore,
    Psql,
    Mysqldump,
    Mysql,
}

impl From<ToolKind> for DumpToolKindDto {
    fn from(kind: ToolKind) -> Self {
        match kind {
            ToolKind::PgDump => Self::PgDump,
            ToolKind::PgRestore => Self::PgRestore,
            ToolKind::Psql => Self::Psql,
            ToolKind::MysqlDump => Self::Mysqldump,
            ToolKind::Mysql => Self::Mysql,
        }
    }
}

/// The copy of a tool a backup or restore uses, and why it may not work.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DumpToolDto {
    pub(crate) tool: DumpToolKindDto,
    /// Its name, e.g. `pg_dump`.
    pub(crate) name: String,
    /// The chosen copy; `None` when none was found.
    pub(crate) path: Option<String>,
    /// What it reported, e.g. `PostgreSQL 17.4`.
    pub(crate) version: Option<String>,
    /// Chosen in Settings ▸ Tools rather than found automatically.
    pub(crate) from_settings: bool,
    /// Why it may not work with the server.
    pub(crate) warning: Option<String>,
    /// Why no copy can be used.
    pub(crate) error: Option<String>,
    /// Other installed copies, e.g. `/usr/local/bin/pg_dump (PostgreSQL 14.17)`.
    pub(crate) others: Vec<String>,
}

/// The session a backup or restore starts from, and its server as the
/// connection reported it.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DumpScopeRequest {
    pub(crate) session_id: String,
    /// `PostgreSQL`, `MySQL`, `MariaDB`, … as the connection reported it.
    pub(crate) server_product: Option<String>,
    /// E.g. `17.2`.
    pub(crate) server_version: Option<String>,
}

impl DumpScopeRequest {
    /// The server, for picking and checking tools.
    pub(crate) fn server(&self, engine: DumpEngineDto) -> Option<ServerVersion> {
        let version = Version::parse(self.server_version.as_deref()?)?;
        let named_mariadb = self
            .server_product
            .as_deref()
            .is_some_and(|product| product.contains("MariaDB"));
        let product = match engine {
            DumpEngineDto::Postgres => Product::Postgres,
            DumpEngineDto::Mysql if named_mariadb => Product::Mariadb,
            DumpEngineDto::Mysql => Product::Mysql,
            DumpEngineDto::Sqlite => return None,
        };
        Some(ServerVersion { product, version })
    }
}

/// What the backup and restore sheets need to know.
#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DumpContextDto {
    pub(crate) engine: DumpEngineDto,
    pub(crate) connection: String,
    pub(crate) database: String,
    /// Restores are refused.
    pub(crate) read_only: bool,
    /// Tagged production: restores need the database name typed.
    pub(crate) production: bool,
    /// The tools of the engine (none for `SQLite`).
    pub(crate) tools: Vec<DumpToolDto>,
}

/// `pg_dump` output format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum BackupFormatDto {
    Plain,
    Custom,
    Directory,
    Tar,
}

/// What a backup holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum BackupContentDto {
    All,
    SchemaOnly,
    DataOnly,
}

/// A table chosen for a backup.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DumpTableDto {
    pub(crate) schema: String,
    pub(crate) name: String,
}

/// Backup options; those the engine lacks are ignored.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent switches, one per tool flag"
)]
pub(crate) struct BackupOptionsDto {
    pub(crate) format: BackupFormatDto,
    pub(crate) content: BackupContentDto,
    pub(crate) schemas: Vec<String>,
    pub(crate) tables: Vec<DumpTableDto>,
    #[serde(default)]
    pub(crate) exclude_table_data: Vec<DumpTableDto>,
    pub(crate) no_owner: bool,
    pub(crate) no_privileges: bool,
    /// 0–9; `None` for the tool's default.
    pub(crate) compression: Option<u8>,
    pub(crate) single_transaction: bool,
    pub(crate) routines: bool,
    pub(crate) triggers: bool,
    pub(crate) events: bool,
}

impl BackupOptionsDto {
    pub(crate) fn options(&self) -> BackupOptions {
        BackupOptions {
            format: self.format.into(),
            content: match self.content {
                BackupContentDto::All => Content::All,
                BackupContentDto::SchemaOnly => Content::SchemaOnly,
                BackupContentDto::DataOnly => Content::DataOnly,
            },
            schemas: self.schemas.clone(),
            tables: self
                .tables
                .iter()
                .map(|table| TableRef {
                    schema: table.schema.clone(),
                    name: table.name.clone(),
                })
                .collect(),
            exclude_table_data: self
                .exclude_table_data
                .iter()
                .map(|table| TableRef {
                    schema: table.schema.clone(),
                    name: table.name.clone(),
                })
                .collect(),
            no_owner: self.no_owner,
            no_privileges: self.no_privileges,
            compression: self.compression,
            single_transaction: self.single_transaction,
            routines: self.routines,
            triggers: self.triggers,
            events: self.events,
        }
    }
}

impl From<BackupFormatDto> for PgFormat {
    fn from(format: BackupFormatDto) -> Self {
        match format {
            BackupFormatDto::Plain => Self::Plain,
            BackupFormatDto::Custom => Self::Custom,
            BackupFormatDto::Directory => Self::Directory,
            BackupFormatDto::Tar => Self::Tar,
        }
    }
}

/// A backup: the options and the file or folder chosen with
/// `dump_backup_pick` (absent while only previewing).
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupRequest {
    pub(crate) scope: DumpScopeRequest,
    pub(crate) options: BackupOptionsDto,
    pub(crate) output: Option<String>,
}

/// Choose where a backup of the session's database goes.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupPickRequest {
    pub(crate) session_id: String,
    pub(crate) format: BackupFormatDto,
}

/// The exact commands that will run (passwords redacted), and warnings.
#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DumpPreviewDto {
    pub(crate) commands: Vec<String>,
    pub(crate) warnings: Vec<String>,
}

/// Where a restore writes.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum RestoreTargetDto {
    /// The session's database.
    Existing,
    /// A new database, created first.
    Create { name: String },
}

/// `pg_restore` drop behaviour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RestoreCleanDto {
    Keep,
    Drop,
    DropIfExists,
}

/// Restore options; archive options are ignored for SQL scripts.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RestoreOptionsDto {
    #[serde(default)]
    pub(crate) schemas: Vec<String>,
    pub(crate) clean: RestoreCleanDto,
    pub(crate) no_owner: bool,
    pub(crate) no_privileges: bool,
    pub(crate) jobs: u16,
    pub(crate) stop_on_error: bool,
    /// Archive entries to restore; `None` for all.
    pub(crate) selected: Option<Vec<u32>>,
}

impl RestoreOptionsDto {
    pub(crate) fn options(&self) -> RestoreOptions {
        RestoreOptions {
            schemas: self.schemas.clone(),
            clean: match self.clean {
                RestoreCleanDto::Keep => Clean::Keep,
                RestoreCleanDto::Drop => Clean::Drop,
                RestoreCleanDto::DropIfExists => Clean::DropIfExists,
            },
            no_owner: self.no_owner,
            no_privileges: self.no_privileges,
            jobs: self.jobs,
            stop_on_error: self.stop_on_error,
            selected: self.selected.clone(),
        }
    }
}

/// A restore of a picked backup.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RestoreRequest {
    pub(crate) scope: DumpScopeRequest,
    pub(crate) path: String,
    pub(crate) target: RestoreTargetDto,
    pub(crate) options: RestoreOptionsDto,
    /// From `dump_restore_check`, once the user confirmed.
    pub(crate) confirm_token: Option<String>,
    /// The database name, typed when the check asked for it.
    pub(crate) typed_name: Option<String>,
}

/// What a picked backup is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum DumpFileKindDto {
    PlainSql,
    PgCustom,
    PgTar,
    PgDirectory,
    Sqlite,
}

impl From<DumpKind> for DumpFileKindDto {
    fn from(kind: DumpKind) -> Self {
        match kind {
            DumpKind::PlainSql => Self::PlainSql,
            DumpKind::PgCustom => Self::PgCustom,
            DumpKind::PgTar => Self::PgTar,
            DumpKind::PgDirectory => Self::PgDirectory,
        }
    }
}

/// A backup chosen in the open panel.
#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PickedDumpDto {
    pub(crate) path: String,
    pub(crate) name: String,
    pub(crate) kind: DumpFileKindDto,
    pub(crate) bytes: f64,
}

/// List the contents of a picked `pg_dump` archive.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RestoreContentsRequest {
    pub(crate) scope: DumpScopeRequest,
    pub(crate) path: String,
}

/// One entry of an archive's table of contents.
#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArchiveEntryDto {
    pub(crate) id: u32,
    pub(crate) label: String,
    pub(crate) data: bool,
}

/// Whether a restore may start and how to confirm it.
#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RestoreCheckDto {
    pub(crate) commands: Vec<String>,
    pub(crate) warnings: Vec<String>,
    /// Refused on this connection (read-only), with the reason.
    pub(crate) blocked: Option<String>,
    /// The database the restore writes to.
    pub(crate) database: String,
    /// The user must type `database` to confirm.
    pub(crate) typed: bool,
    /// Pass back to `dump_restore_start`.
    pub(crate) confirm_token: Option<String>,
}

/// A started backup or restore; `dump_cancel(jobId)` cancels it.
#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DumpStarted {
    pub(crate) job_id: String,
}

/// How a backup or restore ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum DumpOutcomeDto {
    Completed,
    Failed,
    Cancelled,
}

/// Events of a running backup or restore; `finished` ends them.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum DumpEventDto {
    /// A step (create database, then restore) starts with this command.
    Step { index: u32, command: String },
    /// New stderr lines of the tool.
    Log { lines: Vec<String> },
    /// Objects done (of `total` when known) and bytes written or read.
    Progress {
        objects: f64,
        total: Option<f64>,
        bytes: f64,
        total_bytes: Option<f64>,
    },
    Finished {
        outcome: DumpOutcomeDto,
        /// The tool's exit code when it failed.
        exit_code: Option<i32>,
        message: Option<String>,
        /// The backup's file or folder.
        output: Option<String>,
        elapsed_ms: f64,
    },
}
