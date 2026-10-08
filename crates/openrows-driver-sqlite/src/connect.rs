// SPDX-License-Identifier: Apache-2.0
//! Opening a database file (the `SQLite` [`Driver`]): read-only or
//! read-write, created only when asked, extension loading off, a busy
//! timeout so a writer elsewhere does not fail reads at once.
use std::path::Path;
use std::time::Duration;

use openrows_driver_api::{
    BoxFuture, ConnectRequest, ConnectStage, ConnectTrace, Connection, Driver, DriverError,
    Endpoint, ServerSummary, TlsStatus,
};
use openrows_sql::Dialect;
use rusqlite::OpenFlags;

use crate::session::SqliteSession;
use crate::worker::Worker;

/// The one database of a file, as `SQLite` names it.
const MAIN: &str = "main";

/// How long a statement waits for another connection's lock.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Local `SQLite` 3 files.
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
            let Endpoint::File {
                path,
                read_only,
                create,
            } = &request.endpoint
            else {
                return Err(DriverError::Unsupported(
                    "SQLite opens a local file, not a server".to_owned(),
                ));
            };
            let open = async { open(path, *read_only, *create) };
            let session = trace.stage(ConnectStage::Connect, open).await?;
            trace.tls = Some(TlsStatus::LocalSocket);
            Ok(Box::new(session) as Box<dyn Connection>)
        })
    }
}

/// Open `path` on its own thread.
///
/// # Errors
/// `Server` with SQLSTATE `58030` when the file is missing (and `create` is
/// off), unreadable or not a database.
pub fn open(path: &Path, read_only: bool, create: bool) -> Result<SqliteSession, DriverError> {
    if !create && !path.exists() {
        return Err(open_error(
            path,
            "no such file; choose New File to create one",
        ));
    }
    let flags = if read_only {
        OpenFlags::SQLITE_OPEN_READ_ONLY
    } else if create {
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
    } else {
        OpenFlags::SQLITE_OPEN_READ_WRITE
    };
    let connection =
        rusqlite::Connection::open_with_flags(path, flags | OpenFlags::SQLITE_OPEN_NO_MUTEX)
            .map_err(|error| open_error(path, &error.to_string()))?;
    connection
        .busy_timeout(BUSY_TIMEOUT)
        .map_err(|error| open_error(path, &error.to_string()))?;
    // Reading the schema proves the file is a database before the UI says "connected".
    connection
        .query_row("SELECT count(*) FROM main.sqlite_master", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(|error| open_error(path, &error.to_string()))?;
    let interrupt = connection.get_interrupt_handle();
    let summary = ServerSummary {
        product: "SQLite".to_owned(),
        version: rusqlite::version().to_owned(),
        flavour: read_only.then(|| "read-only".to_owned()),
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

fn open_error(path: &Path, reason: &str) -> DriverError {
    DriverError::Server {
        code: "58030".to_owned(),
        message: format!("{}: {reason}", path.display()),
    }
}
