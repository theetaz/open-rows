// SPDX-License-Identifier: Apache-2.0
//! `SQLite` result codes as typed driver errors with SQLSTATE-style codes,
//! so the UI treats constraint, permission and syntax errors like other
//! engines' (docs/04 §7).
use openrows_driver_api::DriverError;
use rusqlite::ffi;

/// SQLSTATE `query_canceled`, as `PostgreSQL` reports a cancelled statement.
pub(crate) const CANCELLED: &str = "57014";

/// A rusqlite failure as a driver error; the connection stays usable.
pub(crate) fn driver_error(error: &rusqlite::Error) -> DriverError {
    let message = error.to_string();
    let code = match error {
        rusqlite::Error::SqliteFailure(failure, _)
        | rusqlite::Error::SqlInputError { error: failure, .. } => sqlstate(*failure, &message),
        rusqlite::Error::InvalidParameterCount(..) | rusqlite::Error::InvalidParameterName(_) => {
            "07001"
        }
        rusqlite::Error::MultipleStatement => "42601",
        _ => "HY000",
    };
    DriverError::Server {
        code: code.to_owned(),
        message,
    }
}

/// The closest SQLSTATE for an `SQLite` result code.
fn sqlstate(failure: ffi::Error, message: &str) -> &'static str {
    match failure.extended_code {
        ffi::SQLITE_CONSTRAINT_UNIQUE | ffi::SQLITE_CONSTRAINT_PRIMARYKEY => return "23505",
        ffi::SQLITE_CONSTRAINT_FOREIGNKEY => return "23503",
        ffi::SQLITE_CONSTRAINT_NOTNULL => return "23502",
        ffi::SQLITE_CONSTRAINT_CHECK => return "23514",
        _ => {}
    }
    match failure.code {
        ffi::ErrorCode::ConstraintViolation => "23000",
        ffi::ErrorCode::OperationInterrupted => CANCELLED,
        ffi::ErrorCode::ReadOnly => "25006",
        ffi::ErrorCode::PermissionDenied | ffi::ErrorCode::AuthorizationForStatementDenied => {
            "42501"
        }
        ffi::ErrorCode::DatabaseBusy | ffi::ErrorCode::DatabaseLocked => "55P03",
        ffi::ErrorCode::CannotOpen | ffi::ErrorCode::SystemIoFailure => "58030",
        ffi::ErrorCode::NotADatabase | ffi::ErrorCode::DatabaseCorrupt => "XX001",
        ffi::ErrorCode::TooBig => "54000",
        _ => by_message(message),
    }
}

/// Generic `SQLITE_ERROR`: the message names syntax and catalog problems.
fn by_message(message: &str) -> &'static str {
    if message.contains("syntax error") || message.contains("incomplete input") {
        "42601"
    } else if message.contains("no such table") {
        "42P01"
    } else if message.contains("no such column") {
        "42703"
    } else if message.contains("no such function") {
        "42883"
    } else if message.contains("already exists") {
        "42P07"
    } else {
        "HY000"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn code(connection: &Connection, sql: &str) -> String {
        match connection
            .execute_batch(sql)
            .map_err(|error| driver_error(&error))
        {
            Err(DriverError::Server { code, .. }) => code,
            other => format!("{other:?}"),
        }
    }

    #[test]
    fn result_codes_map_to_sqlstates() -> Result<(), rusqlite::Error> {
        let connection = Connection::open_in_memory()?;
        connection.execute_batch(
            "CREATE TABLE p (id INTEGER PRIMARY KEY, n TEXT NOT NULL UNIQUE, v INT CHECK (v > 0));
             CREATE TABLE c (p INT REFERENCES p(id));
             PRAGMA foreign_keys = ON;
             INSERT INTO p VALUES (1, 'a', 1);",
        )?;
        assert_eq!(
            code(&connection, "INSERT INTO p VALUES (2, 'a', 1)"),
            "23505"
        );
        assert_eq!(
            code(&connection, "INSERT INTO p VALUES (1, 'b', 1)"),
            "23505"
        );
        assert_eq!(
            code(&connection, "INSERT INTO p VALUES (3, NULL, 1)"),
            "23502"
        );
        assert_eq!(
            code(&connection, "INSERT INTO p VALUES (4, 'c', 0)"),
            "23514"
        );
        assert_eq!(code(&connection, "INSERT INTO c VALUES (9)"), "23503");
        assert_eq!(code(&connection, "SELEC 1"), "42601");
        assert_eq!(code(&connection, "SELECT * FROM missing"), "42P01");
        assert_eq!(code(&connection, "SELECT nope FROM p"), "42703");
        assert_eq!(code(&connection, "CREATE TABLE p (x)"), "42P07");
        Ok(())
    }
}
