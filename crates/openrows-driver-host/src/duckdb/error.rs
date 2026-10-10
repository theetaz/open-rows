// SPDX-License-Identifier: Apache-2.0
//! Preserve engine text while mapping common failures to the shared SQLSTATE contract.
use openrows_driver_api::DriverError;

pub(super) fn map(error: duckdb::Error) -> DriverError {
    let message = match error {
        duckdb::Error::DuckDBFailure(_, Some(message)) => message,
        other => other.to_string(),
    };
    let code = if message.contains("Interrupt") {
        "57014"
    } else if message.starts_with("Parser Error") {
        "42601"
    } else if message.contains("does not exist") {
        "42P01"
    } else if message.contains("NOT NULL constraint") {
        "23502"
    } else if message.contains("Duplicate key") || message.contains("PRIMARY KEY or UNIQUE") {
        "23505"
    } else if message.starts_with("IO Error") {
        "58030"
    } else if message.starts_with("Permission Error") {
        "42501"
    } else {
        "HY000"
    };
    DriverError::Server {
        code: code.into(),
        message,
    }
}

pub(super) fn open(error: duckdb::Error) -> DriverError {
    DriverError::Server {
        code: "58030".into(),
        message: format!(
            "DuckDB could not open the database (another application may own its lock): {}",
            map(error)
        ),
    }
}
