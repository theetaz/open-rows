// SPDX-License-Identifier: Apache-2.0
//! Hrana errors mapped to the same SQLSTATEs as the `SQLite` driver.
use crate::protocol::Error;
use openrows_driver_api::DriverError;

pub(crate) fn protocol() -> DriverError {
    DriverError::Protocol("invalid Hrana 3 response".into())
}

pub(crate) fn server(error: &Error, token: Option<&str>) -> DriverError {
    let message = scrub(&error.message, token);
    let code = error.code.as_deref().unwrap_or("SQLITE_ERROR");
    DriverError::Server {
        code: sqlstate(code, &message).into(),
        message,
    }
}

pub(crate) fn scrub(message: &str, token: Option<&str>) -> String {
    let clean = token.filter(|token| !token.is_empty()).map_or_else(
        || message.to_owned(),
        |token| message.replace(token, "[redacted]"),
    );
    clean.chars().take(1024).collect()
}

pub(crate) fn cancelled() -> DriverError {
    DriverError::Server { code: "57014".into(),
        message: "HTTP request aborted; the server may finish the statement. The stream was closed or abandoned.".into() }
}

pub(crate) fn lost(reason: &str) -> DriverError {
    DriverError::Network(std::io::Error::new(
        std::io::ErrorKind::ConnectionAborted,
        reason,
    ))
}

fn sqlstate(code: &str, message: &str) -> &'static str {
    match code {
        "SQLITE_CONSTRAINT_UNIQUE" | "SQLITE_CONSTRAINT_PRIMARYKEY" => "23505",
        "SQLITE_CONSTRAINT_FOREIGNKEY" => "23503",
        "SQLITE_CONSTRAINT_NOTNULL" => "23502",
        "SQLITE_CONSTRAINT_CHECK" => "23514",
        "SQLITE_CONSTRAINT" => constraint_state(message),
        "SQLITE_INTERRUPT" => "57014",
        "SQLITE_READONLY" => "25006",
        "SQLITE_AUTH" | "SQLITE_PERM" => "42501",
        "SQLITE_BUSY" | "SQLITE_LOCKED" => "55P03",
        "SQLITE_CANTOPEN" | "SQLITE_IOERR" => "58030",
        "SQLITE_NOTADB" | "SQLITE_CORRUPT" => "XX001",
        "SQLITE_TOOBIG" => "54000",
        "ARGS_INVALID" => "07001",
        _ => by_message(message),
    }
}

fn constraint_state(message: &str) -> &'static str {
    if message.contains("UNIQUE constraint failed") {
        "23505"
    } else if message.contains("FOREIGN KEY constraint failed") {
        "23503"
    } else if message.contains("NOT NULL constraint failed") {
        "23502"
    } else if message.contains("CHECK constraint failed") {
        "23514"
    } else {
        "23000"
    }
}

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
    #[test]
    fn maps_constraints_and_scrubs_echoed_token() {
        let error = server(
            &Error {
                message: "test-only syntax error".into(),
                code: Some("SQLITE_ERROR".into()),
            },
            Some("test-only"),
        );
        assert!(
            matches!(error, DriverError::Server { code, message } if code == "42601" && !message.contains("test-only"))
        );
        for (source, target) in [
            ("SQLITE_CONSTRAINT_UNIQUE", "23505"),
            ("SQLITE_CONSTRAINT_NOTNULL", "23502"),
            ("SQLITE_INTERRUPT", "57014"),
            ("SQLITE_READONLY", "25006"),
        ] {
            assert_eq!(sqlstate(source, "x"), target);
        }
        for (message, expected) in [
            ("UNIQUE constraint failed: t.id", "23505"),
            ("NOT NULL constraint failed: t.x", "23502"),
            ("FOREIGN KEY constraint failed", "23503"),
            ("CHECK constraint failed: x > 0", "23514"),
        ] {
            assert_eq!(sqlstate("SQLITE_CONSTRAINT", message), expected);
        }
    }
}
