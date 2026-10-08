// SPDX-License-Identifier: Apache-2.0
//! `MySQL` `SET`: user variables and harmless session variables read;
//! anything global, persistent, privilege- or transaction-related is
//! `Unknown`, so safe mode treats it as a write.

use super::{Sql, StatementKind};
use crate::lexer::TokenKind;

use StatementKind::{Read, TxControl, Unknown};

/// Session variables whose `SET` cannot modify data or lift a read-only guard.
const HARMLESS: &[&str] = &[
    "autocommit",
    "big_tables",
    "character_set_client",
    "character_set_connection",
    "character_set_results",
    "collation_connection",
    "group_concat_max_len",
    "innodb_lock_wait_timeout",
    "interactive_timeout",
    "lock_wait_timeout",
    "max_execution_time",
    "max_statement_time",
    "net_read_timeout",
    "net_write_timeout",
    "profiling",
    "sql_select_limit",
    "time_zone",
    "wait_timeout",
];

/// Classify `SET …`; every comma-separated assignment must be harmless.
pub(super) fn set(sql: Sql<'_>) -> StatementKind {
    let first = sql.word(1).unwrap_or_default();
    let transaction = first == "TRANSACTION"
        || (matches!(first.as_str(), "SESSION" | "LOCAL") && sql.is_word(2, "TRANSACTION"));
    if transaction {
        return if sql.has_word_pair("READ", "WRITE") {
            Unknown
        } else {
            TxControl
        };
    }
    if matches!(first.as_str(), "NAMES" | "CHARSET" | "CHARACTER") {
        return Read;
    }
    let starts = assignment_starts(sql);
    if starts.iter().all(|&index| harmless(sql, index)) {
        Read
    } else {
        Unknown
    }
}

/// Token index of each assignment: after `SET` and after every top-level comma.
fn assignment_starts(sql: Sql<'_>) -> Vec<usize> {
    let mut depth = 0_usize;
    let mut starts = vec![1];
    for (index, token) in sql.tokens.iter().enumerate() {
        match token.kind {
            TokenKind::OpenParen => depth += 1,
            TokenKind::CloseParen => depth = depth.saturating_sub(1),
            TokenKind::Punct if depth == 0 && token.text(sql.source) == "," => {
                starts.push(index + 1);
            }
            _ => {}
        }
    }
    starts
}

/// `@user_var`, or a harmless session variable written as `name`,
/// `SESSION name`, `LOCAL name`, `@@name`, `@@session.name` or `@@local.name`.
fn harmless(sql: Sql<'_>, index: usize) -> bool {
    let at = |offset: usize| sql.text(index + offset) == "@";
    if at(0) && !at(1) {
        return true;
    }
    let mut name = if at(0) && at(1) { index + 2 } else { index };
    let scope = sql.word(name).unwrap_or_default();
    if matches!(scope.as_str(), "SESSION" | "LOCAL") {
        name += if sql.text(name + 1) == "." { 2 } else { 1 };
    }
    sql.word(name).is_some_and(|word| {
        HARMLESS
            .iter()
            .any(|known| word.eq_ignore_ascii_case(known))
    })
}
