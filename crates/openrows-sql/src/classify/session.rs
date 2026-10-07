// SPDX-License-Identifier: Apache-2.0
//! Transaction control and session settings (`BEGIN`, `SET`, `RESET`, …).
//!
//! Anything that could lift a server-side read-only guard (`READ WRITE`
//! transactions, `SET ROLE`, `SET default_transaction_read_only`, `RESET ALL`)
//! is `Unknown`, so safe mode treats it as a write.

use super::{Classification, Sql, StatementKind};

use StatementKind::{Read, TxControl, Unknown};

/// Run-time parameters whose `SET`/`RESET` cannot modify data or lift a
/// read-only guard. Every other parameter is `Unknown`.
const HARMLESS_SETTINGS: &[&str] = &[
    "application_name",
    "bytea_output",
    "client_encoding",
    "client_min_messages",
    "datestyle",
    "extra_float_digits",
    "idle_in_transaction_session_timeout",
    "intervalstyle",
    "lock_timeout",
    "names",
    "schema",
    "search_path",
    "statement_timeout",
    "time",
    "timezone",
    "work_mem",
];

/// Classify heads not covered by DML/DDL rules; unrecognised heads (`CALL`,
/// `DO`, `EXECUTE`, `DISCARD`, `LOAD`, …) are `Unknown`.
pub(super) fn dispatch(sql: Sql<'_>, head: &str) -> Classification {
    let kind = match head {
        "BEGIN" | "START" => transaction_mode(sql),
        "COMMIT" | "END" | "ROLLBACK" | "ABORT" | "SAVEPOINT" | "RELEASE" => TxControl,
        "SET" => set(sql),
        "RESET" => setting(sql, 1),
        _ => Unknown,
    };
    Classification::new(kind)
}

/// Transaction control, unless it explicitly asks for `READ WRITE`.
fn transaction_mode(sql: Sql<'_>) -> StatementKind {
    if sql.has_word_pair("READ", "WRITE") {
        Unknown
    } else {
        TxControl
    }
}

fn set(sql: Sql<'_>) -> StatementKind {
    let first = sql.word(1).unwrap_or_default();
    if first == "TRANSACTION" || (first == "SESSION" && sql.is_word(2, "CHARACTERISTICS")) {
        return transaction_mode(sql);
    }
    let target = if first == "SESSION" || first == "LOCAL" {
        2
    } else {
        1
    };
    setting(sql, target)
}

/// `Read` when the token at `index` names a harmless, non-qualified setting.
fn setting(sql: Sql<'_>, index: usize) -> StatementKind {
    let qualified = sql.text(index + 1) == ".";
    let harmless = sql.word(index).is_some_and(|name| {
        HARMLESS_SETTINGS
            .iter()
            .any(|known| name.eq_ignore_ascii_case(known))
    });
    if harmless && !qualified {
        Read
    } else {
        Unknown
    }
}
