// SPDX-License-Identifier: Apache-2.0
//! Opening a database (the `SQLite` [`Driver`]): a file read-only or
//! read-write, created only when asked, or a process-shared in-memory
//! database (`memdb` VFS); then the profile's attached files. Extension
//! loading stays off; a busy timeout means a writer elsewhere does not fail
//! reads at once.
use std::path::Path;
use std::time::Duration;

use openrows_driver_api::{
    Attachment, BoxFuture, ConnectRequest, ConnectStage, ConnectTrace, Connection, Driver,
    DriverError, Endpoint, FileSource, ServerSummary, TlsStatus,
};
use openrows_sql::Dialect;
use openrows_sql::attach::attach_sql;
use rusqlite::OpenFlags;

use crate::session::SqliteSession;
use crate::worker::Worker;

/// The one database of a file, as `SQLite` names it.
const MAIN: &str = "main";

/// How long a statement waits for another connection's lock.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// SQLSTATE-style code for a database that cannot be opened or attached.
const OPEN_FAILED: &str = "58030";

/// Local `SQLite` 3 files and in-memory databases.
#[derive(Clone, Copy, Debug, Default)]
pub struct SqliteDriver;

impl Driver for SqliteDriver {
    fn product(&self) -> &'static str {
        "SQLite"
    }

    fn dialect(&self) -> Dialect {
        Dialect::Sqlite
    }

    fn home_database(&self) -> &'static str {
        MAIN
    }

    fn connect<'a>(
        &'a self,
        request: &'a ConnectRequest,
        trace: &'a mut ConnectTrace,
    ) -> BoxFuture<'a, Result<Box<dyn Connection>, DriverError>> {
        Box::pin(async move {
            let Endpoint::File { source, attached } = &request.endpoint else {
                return Err(DriverError::Unsupported(
                    "SQLite opens a local file, not a server".to_owned(),
                ));
            };
            let open = async { open_source(source, attached) };
            let session = trace.stage(ConnectStage::Connect, open).await?;
            trace.tls = Some(TlsStatus::LocalSocket);
            Ok(Box::new(session) as Box<dyn Connection>)
        })
    }
}

/// Open the file at `path` on its own thread, attaching nothing.
///
/// # Errors
/// As [`open_source`].
pub fn open(path: &Path, read_only: bool, create: bool) -> Result<SqliteSession, DriverError> {
    let source = FileSource::Disk {
        path: path.to_path_buf(),
        read_only,
        create,
    };
    open_source(&source, &[])
}

/// Open `source` on its own thread and attach `attached` in order, with the
/// main database's read-only-ness.
///
/// # Errors
/// `Server` with SQLSTATE `58030` when the file is missing (and `create` is
/// off), unreadable or not a database, when the in-memory name is not
/// usable, or when an attachment fails (the message names its alias and file).
pub fn open_source(
    source: &FileSource,
    attached: &[Attachment],
) -> Result<SqliteSession, DriverError> {
    let connection = open_connection(source)?;
    for attachment in attached {
        attach(&connection, attachment, source.read_only())?;
    }
    let interrupt = connection.get_interrupt_handle();
    let flavour = match source {
        FileSource::Memory { .. } => Some("in-memory"),
        FileSource::Disk { read_only, .. } => read_only.then_some("read-only"),
    };
    let summary = ServerSummary {
        product: "SQLite".to_owned(),
        version: rusqlite::version().to_owned(),
        flavour: flavour.map(str::to_owned),
        database: MAIN.to_owned(),
        user: String::new(),
        tls: TlsStatus::LocalSocket,
    };
    Ok(SqliteSession::new(
        Worker::spawn(connection)?,
        interrupt,
        summary,
    ))
}

fn open_connection(source: &FileSource) -> Result<rusqlite::Connection, DriverError> {
    let failed = |reason: &str| open_error(&source.describe(), reason);
    let (target, flags) = match source {
        FileSource::Disk {
            path,
            read_only,
            create,
        } => {
            if !create && !path.exists() {
                return Err(failed("no such file; choose New File to create one"));
            }
            (path.display().to_string(), disk_flags(*read_only, *create))
        }
        FileSource::Memory { name } => (
            memory_uri(name).ok_or_else(|| failed("the in-memory name is not usable"))?,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        ),
    };
    // URI names let ATTACH open files with `?mode=ro`; plain paths are unaffected.
    let flags = flags | OpenFlags::SQLITE_OPEN_URI | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let connection = rusqlite::Connection::open_with_flags(target, flags)
        .map_err(|error| failed(&error.to_string()))?;
    connection
        .busy_timeout(BUSY_TIMEOUT)
        .map_err(|error| failed(&error.to_string()))?;
    // Reading the schema proves the file is a database before the UI says "connected".
    connection
        .query_row("SELECT count(*) FROM main.sqlite_master", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(|error| failed(&error.to_string()))?;
    Ok(connection)
}

fn disk_flags(read_only: bool, create: bool) -> OpenFlags {
    if read_only {
        OpenFlags::SQLITE_OPEN_READ_ONLY
    } else if create {
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
    } else {
        OpenFlags::SQLITE_OPEN_READ_WRITE
    }
}

/// `file:/name?vfs=memdb`: the leading `/` shares the database with every
/// connection in this process that opens the same name.
fn memory_uri(name: &str) -> Option<String> {
    let usable = !name.is_empty()
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'));
    usable.then(|| format!("file:/{name}?vfs=memdb"))
}

fn attach(
    connection: &rusqlite::Connection,
    attachment: &Attachment,
    read_only: bool,
) -> Result<(), DriverError> {
    let failed = |reason: &str| {
        let what = format!(
            "attaching {} ({})",
            attachment.alias,
            attachment.path.display()
        );
        open_error(&what, reason)
    };
    if !attachment.path.exists() {
        return Err(failed("no such file"));
    }
    let bound = attach_sql(&attachment.alias, &attachment.path, read_only)
        .map_err(|error| failed(&error.to_string()))?;
    connection
        .execute(&bound.sql, rusqlite::params_from_iter(&bound.params))
        .map_err(|error| failed(&error.to_string()))?;
    Ok(())
}

fn open_error(what: &str, reason: &str) -> DriverError {
    DriverError::Server {
        code: OPEN_FAILED.to_owned(),
        message: format!("{what}: {reason}"),
    }
}
