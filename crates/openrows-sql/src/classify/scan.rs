// SPDX-License-Identifier: Apache-2.0
//! Whole-statement scans that escalate `Read` statements hiding writes and
//! find unbounded `DELETE`/`UPDATE` anywhere in a statement.

use super::{Sql, tsql};
use crate::Dialect;
use crate::lexer::TokenKind;

/// Data-modifying keywords when they start a (sub)statement.
const DML_HEADS: &[&str] = &["INSERT", "UPDATE", "DELETE", "MERGE"];

/// Locking clauses after `FOR` (`FOR UPDATE`, `FOR NO KEY UPDATE`, `FOR SHARE`, `FOR KEY SHARE`).
const LOCK_STRENGTHS: &[&str] = &["UPDATE", "NO", "SHARE", "KEY"];

/// `MySQL`/`MariaDB` built-ins with side effects (locks, sequences, UDF shells).
const MYSQL_SIDE_EFFECTS: &[&str] = &[
    "get_lock",
    "release_lock",
    "release_all_locks",
    "nextval",
    "setval",
    "sys_exec",
    "sys_eval",
];

/// `SQLite` functions with side effects (extension loading, CLI file writes).
const SQLITE_SIDE_EFFECTS: &[&str] = &["load_extension", "writefile", "edit"];

/// `PostgreSQL` built-ins with side effects. Heuristic: user-defined volatile
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
/// `SELECT … INTO`, a locking clause (`FOR UPDATE`, `MySQL` `LOCK IN SHARE
/// MODE`), or a side-effecting function call.
pub(super) fn has_write_marker(sql: Sql<'_>) -> bool {
    (0..sql.tokens.len()).any(|index| {
        (is_head(sql, index) && (is_any_word(sql, index, DML_HEADS) || is_replace(sql, index)))
            || sql.is_word(index, "INTO")
            || (sql.is_word(index, "FOR") && is_any_word(sql, index + 1, LOCK_STRENGTHS))
            || (sql.is_word(index, "LOCK") && sql.is_word(index + 1, "IN"))
            || is_side_effect_call(sql, index)
    })
}

/// Whether any `DELETE` or `UPDATE` that starts a (sub)statement lacks a
/// `WHERE` at its own nesting level. A SQL Server batch needs no `;`
/// between statements, so there any data-statement `DELETE`/`UPDATE`
/// counts and its `WHERE` must come before the next statement.
pub(super) fn has_unbounded_dml(sql: Sql<'_>) -> bool {
    (0..sql.tokens.len()).any(|index| {
        let dml = if sql.dialect == Dialect::SqlServer {
            tsql::dml_head(sql, index, "DELETE") || tsql::dml_head(sql, index, "UPDATE")
        } else {
            is_head(sql, index) && is_any_word(sql, index, &["DELETE", "UPDATE"])
        };
        dml && !has_where(sql, index + 1)
    })
}

/// Words that start the next statement of a SQL Server batch; a `WHERE`
/// after one of them belongs to that statement.
const TSQL_STATEMENT_HEADS: &[&str] = &[
    "SELECT", "INSERT", "MERGE", "EXEC", "EXECUTE", "DECLARE", "IF", "WHILE", "PRINT", "BEGIN",
    "TRUNCATE", "DROP", "CREATE", "ALTER",
];

/// `REPLACE [INTO] t …` (`MySQL`, `SQLite`), not the `replace(…)` function.
fn is_replace(sql: Sql<'_>, index: usize) -> bool {
    matches!(sql.dialect, Dialect::MySql | Dialect::Sqlite)
        && sql.is_word(index, "REPLACE")
        && sql.kind(index + 1) != Some(TokenKind::OpenParen)
}

/// A (sub)statement starts at the beginning, after `(`, after a stray `;`,
/// or, inside an anonymous block that runs at once (`MariaDB` `BEGIN NOT
/// ATOMIC … END`), after `ATOMIC`, `BEGIN`, `THEN` or `ELSE`, so DML hidden
/// in the block is still inspected.
fn is_head(sql: Sql<'_>, index: usize) -> bool {
    let Some(previous) = index.checked_sub(1) else {
        return true;
    };
    matches!(
        sql.kind(previous),
        Some(TokenKind::OpenParen | TokenKind::Semicolon)
    ) || (sql.is_word(0, "BEGIN")
        && is_any_word(sql, previous, &["BEGIN", "ATOMIC", "THEN", "ELSE"]))
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
            TokenKind::Word if depth == 0 && next_tsql_statement(sql, index) => return false,
            _ => {}
        }
    }
    false
}

/// A SQL Server statement head at `index`, ending the previous statement.
fn next_tsql_statement(sql: Sql<'_>, index: usize) -> bool {
    sql.dialect == Dialect::SqlServer
        && (is_any_word(sql, index, TSQL_STATEMENT_HEADS)
            || tsql::dml_head(sql, index, "DELETE")
            || tsql::dml_head(sql, index, "UPDATE"))
}

fn is_side_effect_call(sql: Sql<'_>, index: usize) -> bool {
    if sql.kind(index + 1) != Some(TokenKind::OpenParen) {
        return false;
    }
    let Some(name) = sql.word(index) else {
        return false;
    };
    let name = name.to_ascii_lowercase();
    match sql.dialect {
        Dialect::Postgres | Dialect::Duckdb => {
            SIDE_EFFECT_FUNCTIONS.contains(&name.as_str())
                || SIDE_EFFECT_PREFIXES
                    .iter()
                    .any(|prefix| name.starts_with(prefix))
        }
        Dialect::MySql => MYSQL_SIDE_EFFECTS.contains(&name.as_str()),
        Dialect::Sqlite => SQLITE_SIDE_EFFECTS.contains(&name.as_str()),
        // T-SQL functions cannot change data; sequences are `NEXT VALUE FOR`.
        Dialect::SqlServer | Dialect::ClickHouse => false,
    }
}
