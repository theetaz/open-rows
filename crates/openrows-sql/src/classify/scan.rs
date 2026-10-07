// SPDX-License-Identifier: Apache-2.0
//! Whole-statement scans that escalate `Read` statements hiding writes and
//! find unbounded `DELETE`/`UPDATE` anywhere in a statement.

use super::Sql;
use crate::lexer::TokenKind;

/// Data-modifying keywords when they start a (sub)statement.
const DML_HEADS: &[&str] = &["INSERT", "UPDATE", "DELETE", "MERGE"];

/// Locking clauses after `FOR` (`FOR UPDATE`, `FOR NO KEY UPDATE`, `FOR SHARE`, `FOR KEY SHARE`).
const LOCK_STRENGTHS: &[&str] = &["UPDATE", "NO", "SHARE", "KEY"];

/// Built-in functions with side effects. Heuristic: user-defined volatile
/// functions cannot be detected without the catalog.
const SIDE_EFFECT_FUNCTIONS: &[&str] = &[
    "nextval",
    "setval",
    "set_config",
    "pg_terminate_backend",
    "pg_cancel_backend",
    "pg_reload_conf",
    "pg_rotate_logfile",
    "pg_switch_wal",
    "pg_promote",
    "pg_create_restore_point",
    "pg_notify",
    "pg_file_write",
    "pg_file_unlink",
    "pg_file_rename",
    "pg_logical_slot_get_changes",
    "pg_logical_slot_get_binary_changes",
];

/// Prefixes of side-effecting built-in function families.
const SIDE_EFFECT_PREFIXES: &[&str] = &[
    "lo_",
    "dblink",
    "pg_advisory",
    "pg_try_advisory",
    "pg_stat_reset",
    "pg_create_",
    "pg_drop_",
    "pg_replication_origin_",
];

/// Whether a statement whose head reads actually writes: nested DML,
/// `SELECT … INTO`, a locking clause, or a side-effecting function call.
pub(super) fn has_write_marker(sql: Sql<'_>) -> bool {
    (0..sql.tokens.len()).any(|index| {
        (is_head(sql, index) && is_any_word(sql, index, DML_HEADS))
            || sql.is_word(index, "INTO")
            || (sql.is_word(index, "FOR") && is_any_word(sql, index + 1, LOCK_STRENGTHS))
            || is_side_effect_call(sql, index)
    })
}

/// Whether any `DELETE` or `UPDATE` that starts a (sub)statement lacks a
/// `WHERE` at its own nesting level.
pub(super) fn has_unbounded_dml(sql: Sql<'_>) -> bool {
    (0..sql.tokens.len()).any(|index| {
        is_head(sql, index)
            && is_any_word(sql, index, &["DELETE", "UPDATE"])
            && !has_where(sql, index + 1)
    })
}

/// A (sub)statement starts at the beginning, after `(` or after a stray `;`.
fn is_head(sql: Sql<'_>, index: usize) -> bool {
    index == 0
        || matches!(
            sql.kind(index - 1),
            Some(TokenKind::OpenParen | TokenKind::Semicolon)
        )
}

fn is_any_word(sql: Sql<'_>, index: usize, keywords: &[&str]) -> bool {
    keywords.iter().any(|keyword| sql.is_word(index, keyword))
}

/// Scan forward from `from` for `WHERE` until the enclosing group closes.
fn has_where(sql: Sql<'_>, from: usize) -> bool {
    let mut depth = 0_usize;
    for (index, token) in sql.tokens.iter().enumerate().skip(from) {
        match token.kind {
            TokenKind::OpenParen => depth += 1,
            TokenKind::CloseParen | TokenKind::Semicolon if depth == 0 => return false,
            TokenKind::CloseParen => depth -= 1,
            TokenKind::Word if depth == 0 && sql.is_word(index, "WHERE") => return true,
            _ => {}
        }
    }
    false
}

fn is_side_effect_call(sql: Sql<'_>, index: usize) -> bool {
    if sql.kind(index + 1) != Some(TokenKind::OpenParen) {
        return false;
    }
    let Some(name) = sql.word(index) else {
        return false;
    };
    let name = name.to_ascii_lowercase();
    SIDE_EFFECT_FUNCTIONS.contains(&name.as_str())
        || SIDE_EFFECT_PREFIXES
            .iter()
            .any(|prefix| name.starts_with(prefix))
}
