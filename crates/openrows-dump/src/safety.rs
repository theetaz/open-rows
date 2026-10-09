// SPDX-License-Identifier: Apache-2.0
//! The restore gate. A restore writes to the database, so it is refused on
//! read-only connections and always confirmed; production connections,
//! `clean` (which drops objects first) and replacing a whole `SQLite`
//! file need the database name typed as well.
use std::hash::{DefaultHasher, Hash, Hasher};

use crate::error::DumpError;

/// What the connection and the restore are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RestoreRisk {
    /// The connection refuses writes.
    pub read_only: bool,
    /// The connection is tagged production.
    pub production: bool,
    /// Existing objects are dropped first, or the whole file is replaced.
    pub destructive: bool,
}

/// What the user must do before the restore may start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RestoreGate {
    /// Never on this connection.
    Blocked,
    /// Confirm; with `typed`, by typing the target database's name.
    Confirm {
        /// The database name must be typed.
        typed: bool,
    },
}

/// The gate for `risk`.
#[must_use]
pub fn restore_gate(risk: RestoreRisk) -> RestoreGate {
    if risk.read_only {
        RestoreGate::Blocked
    } else {
        RestoreGate::Confirm {
            typed: risk.production || risk.destructive,
        }
    }
}

/// A token binding a confirmation to exactly this restore (connection,
/// target, file and options): any change yields a different token.
#[must_use]
pub fn confirmation_token(restore: &impl Hash) -> String {
    let mut hasher = DefaultHasher::new();
    restore.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// What the user confirmed.
#[derive(Clone, Copy, Debug)]
pub struct Confirmation<'a> {
    /// The token the check returned, sent back on start.
    pub token: Option<&'a str>,
    /// The database name the user typed, when asked.
    pub typed: Option<&'a str>,
}

/// Admit a restore when the gate allows it and the user confirmed it.
///
/// # Errors
/// `Invalid` explaining what is missing.
pub fn admit(
    gate: RestoreGate,
    expected: &str,
    database: &str,
    given: Confirmation<'_>,
) -> Result<(), DumpError> {
    let typed = match gate {
        RestoreGate::Blocked => {
            return Err(DumpError::invalid(
                "This connection is read-only: restoring would modify the database",
            ));
        }
        RestoreGate::Confirm { typed } => typed,
    };
    if given.token != Some(expected) {
        return Err(DumpError::invalid("Confirm the restore first"));
    }
    if typed && given.typed.map(str::trim) != Some(database) {
        return Err(DumpError::invalid(format!(
            "Type the database name \"{database}\" to confirm the restore"
        )));
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests/safety.rs"]
mod tests;
