// SPDX-License-Identifier: Apache-2.0
//! Server messages as typed driver errors (docs/04 §7). TDS carries error
//! numbers, not SQLSTATEs, so common numbers map to the SQLSTATEs
//! Microsoft's ODBC driver reports (`42000` syntax, `42S02` unknown object,
//! `23000` constraint, `HY008` cancelled); the rest are `HY000`. Login
//! refusals are `Auth`. Messages keep the error number and line.

use std::fmt::Write as _;

use openrows_driver_api::DriverError;

use crate::tds::token::Message;

/// Error numbers that mean the login was refused.
const AUTH_ERRORS: &[i32] = &[
    4060,  // Cannot open the database requested by the login
    18452, // Login from an untrusted domain
    18456, // Login failed for user
    18470, // Account is disabled
    18486, // Account is locked out
    18487, // Password expired
    18488, // Password must be changed
];

/// SQLSTATE of the error numbers worth telling apart.
fn sqlstate(number: i32) -> &'static str {
    match number {
        // 2812: an unknown word at the start of a batch is a procedure call.
        102 | 105 | 156 | 170 | 229 | 230 | 262 | 1038 | 2812 | 4145 => "42000",
        207 => "42S22",
        208 => "42S02",
        2714 => "42S01",
        515 | 547 | 2601 | 2627 => "23000",
        245 | 8114 => "22018",
        2628 | 8152 => "22001",
        220 | 232 | 8115 => "22003",
        8134 => "22012",
        // The attention arrived before the batch started.
        3980 => "HY008",
        241 | 242 | 295 => "22007",
        1205 => "40001",
        3902 | 3903 => "25000",
        6106 => "42704",
        _ => "HY000",
    }
}

/// The statement error the server reported.
pub(crate) fn server_error(message: &Message) -> DriverError {
    DriverError::Server {
        code: sqlstate(message.number).to_owned(),
        message: describe(message),
    }
}

/// A login refusal is `Auth`; anything else during login is a server error.
pub(crate) fn login_error(message: &Message) -> DriverError {
    if AUTH_ERRORS.contains(&message.number) {
        DriverError::Auth(describe(message))
    } else {
        server_error(message)
    }
}

/// The attention signal ended the statement.
pub(crate) fn cancelled() -> DriverError {
    DriverError::Server {
        code: "HY008".to_owned(),
        message: "The statement was cancelled.".to_owned(),
    }
}

/// `text (SQL Server error N, procedure p, line L)`.
fn describe(message: &Message) -> String {
    let mut place = format!("SQL Server error {}", message.number);
    if !message.procedure.is_empty() {
        let _ = write!(place, ", procedure {}", message.procedure);
    }
    if message.line > 0 {
        let _ = write!(place, ", line {}", message.line);
    }
    format!("{} ({place})", message.text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(number: i32, text: &str) -> Message {
        Message {
            error: true,
            number,
            class: 16,
            text: text.to_owned(),
            line: 1,
            ..Message::default()
        }
    }

    #[test]
    fn numbers_map_to_sqlstates_and_logins_to_auth() {
        let syntax = server_error(&message(102, "Incorrect syntax near 'x'."));
        assert!(matches!(
            syntax,
            DriverError::Server { code, message } if code == "42000"
                && message == "Incorrect syntax near 'x'. (SQL Server error 102, line 1)"
        ));
        assert!(
            matches!(server_error(&message(208, "x")), DriverError::Server { code, .. } if code == "42S02")
        );
        assert!(
            matches!(server_error(&message(2627, "x")), DriverError::Server { code, .. } if code == "23000")
        );
        assert!(
            matches!(server_error(&message(50_000, "x")), DriverError::Server { code, .. } if code == "HY000")
        );
        assert!(matches!(
            login_error(&message(18_456, "Login failed")),
            DriverError::Auth(_)
        ));
        assert!(matches!(
            login_error(&message(208, "x")),
            DriverError::Server { .. }
        ));
        assert!(matches!(cancelled(), DriverError::Server { code, .. } if code == "HY008"));
    }
}
