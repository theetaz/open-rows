// SPDX-License-Identifier: Apache-2.0
//! Automatic row limit for unbounded `SELECT`s (FR-EDIT-15, the editor's
//! execution defaults): `SELECT * FROM t` runs as `SELECT * FROM t LIMIT 100`.
//!
//! Only plain queries are rewritten. A statement keeps its text when it is
//! not a `SELECT` (or a read-only `WITH … SELECT`), or when its top level
//! (outside parentheses, strings and comments) already bounds or redirects
//! the rows: `LIMIT`, `OFFSET`, `FETCH`, `TOP`, `INTO`, `FOR` (locking) or
//! `PROCEDURE`/`LOCK` (`MySQL`). The clause goes after the last token, so a
//! trailing comment never swallows it.
use crate::lexer::{Lexer, Token, TokenKind};
use crate::{Dialect, StatementKind, classify};

mod top;

/// Top-level words after which appending `LIMIT n` would be redundant,
/// invalid, or change what the statement does.
const BOUNDED: &[&str] = &[
    "limit",
    "offset",
    "fetch",
    "top",
    "into",
    "for",
    "procedure",
    "lock",
];

/// `statement` with `LIMIT rows` appended when it is an unbounded `SELECT`
/// (SQL Server: `TOP (rows)` after its `SELECT`, see [`top`]); `None` when
/// it runs as written.
///
/// ```
/// use openrows_sql::{Dialect, limit_select};
///
/// let limited = limit_select("SELECT * FROM t -- all", Dialect::Postgres, 100);
/// assert_eq!(limited.as_deref(), Some("SELECT * FROM t LIMIT 100 -- all"));
/// assert_eq!(limit_select("SELECT 1 LIMIT 5", Dialect::Postgres, 100), None);
/// assert_eq!(limit_select("DELETE FROM t", Dialect::Postgres, 100), None);
/// ```
#[must_use]
pub fn limit_select(statement: &str, dialect: Dialect, rows: u64) -> Option<String> {
    if dialect == Dialect::SqlServer {
        return top::limit(statement, rows);
    }
    if !matches!(
        dialect,
        Dialect::Postgres
            | Dialect::Duckdb
            | Dialect::MySql
            | Dialect::Sqlite
            | Dialect::ClickHouse
    ) {
        return None;
    }
    let tokens: Vec<Token> = Lexer::new(statement, dialect)
        .filter(|token| !token.kind.is_trivia())
        .collect();
    if !is_query(statement, dialect, &tokens) || is_bounded(statement, &tokens) {
        return None;
    }
    let end = if dialect == Dialect::ClickHouse {
        clickhouse_end(statement, &tokens)?
    } else {
        tokens.last()?.end
    };
    let (head, tail) = statement.split_at(end);
    Some(format!("{head} LIMIT {rows}{tail}"))
}

fn clickhouse_end(statement: &str, tokens: &[Token]) -> Option<usize> {
    let mut depth = 0_usize;
    for token in tokens {
        match token.kind {
            TokenKind::OpenParen => depth += 1,
            TokenKind::CloseParen => depth = depth.saturating_sub(1),
            TokenKind::Word if depth == 0 => {
                if token.is_word(statement, "settings") || token.is_word(statement, "format") {
                    return Some(statement.get(..token.start)?.trim_end().len());
                }
            }
            _ => {}
        }
    }
    tokens.last().map(|token| token.end)
}

/// A `SELECT`, or a `WITH` whose main statement only reads.
fn is_query(statement: &str, dialect: Dialect, tokens: &[Token]) -> bool {
    let Some(first) = tokens.first() else {
        return false;
    };
    if first.is_word(statement, "select") {
        return true;
    }
    first.is_word(statement, "with") && classify(statement, dialect).kind == StatementKind::Read
}

/// A top-level word of [`BOUNDED`], or a trailing `;` the splitter left
/// (only one statement may be limited at a time).
pub(crate) fn is_bounded(statement: &str, tokens: &[Token]) -> bool {
    let mut depth = 0_usize;
    for token in tokens {
        match token.kind {
            TokenKind::OpenParen => depth += 1,
            TokenKind::CloseParen => depth = depth.saturating_sub(1),
            TokenKind::Semicolon => return true,
            TokenKind::Word if depth == 0 => {
                let word = token.text(statement);
                if BOUNDED.iter().any(|bound| word.eq_ignore_ascii_case(bound)) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIALECTS: [Dialect; 3] = [Dialect::Postgres, Dialect::MySql, Dialect::Sqlite];

    #[test]
    fn appends_limit_to_unbounded_selects_in_every_dialect() {
        for dialect in DIALECTS {
            for (sql, expected) in [
                ("SELECT * FROM t", "SELECT * FROM t LIMIT 50"),
                (
                    "select a from t where b = 1",
                    "select a from t where b = 1 LIMIT 50",
                ),
                (
                    "SELECT 1\n-- trailing note",
                    "SELECT 1 LIMIT 50\n-- trailing note",
                ),
                (
                    "SELECT a FROM t /* c */  ",
                    "SELECT a FROM t LIMIT 50 /* c */  ",
                ),
                (
                    "SELECT * FROM (SELECT * FROM t LIMIT 5) s",
                    "SELECT * FROM (SELECT * FROM t LIMIT 5) s LIMIT 50",
                ),
                (
                    "SELECT a FROM t UNION SELECT a FROM u",
                    "SELECT a FROM t UNION SELECT a FROM u LIMIT 50",
                ),
                (
                    "WITH x AS (SELECT 1) SELECT * FROM x",
                    "WITH x AS (SELECT 1) SELECT * FROM x LIMIT 50",
                ),
            ] {
                assert_eq!(
                    limit_select(sql, dialect, 50).as_deref(),
                    Some(expected),
                    "{dialect:?}: {sql}"
                );
            }
        }
    }

    #[test]
    fn keeps_bounded_and_non_select_statements() {
        for dialect in DIALECTS {
            for sql in [
                "SELECT * FROM t LIMIT 10",
                "SELECT * FROM t limit 10 offset 5",
                "SELECT * FROM t OFFSET 5",
                "SELECT * FROM t FETCH FIRST 3 ROWS ONLY",
                "SELECT * INTO copy FROM t",
                "SELECT * FROM t FOR UPDATE",
                "INSERT INTO t SELECT * FROM u",
                "UPDATE t SET a = 1",
                "DELETE FROM t",
                "EXPLAIN SELECT * FROM t",
                "VALUES (1), (2)",
                "(SELECT 1)",
                "SELECT 1;",
                "",
                "-- only a comment",
            ] {
                assert_eq!(limit_select(sql, dialect, 50), None, "{dialect:?}: {sql}");
            }
        }
    }

    #[test]
    fn words_inside_strings_identifiers_and_comments_do_not_count() {
        let rows = [
            (
                Dialect::Postgres,
                "SELECT 'limit' AS \"limit\" FROM t -- limit",
            ),
            (Dialect::MySql, "SELECT `limit`, \"for\" FROM t # limit"),
            (Dialect::Sqlite, "SELECT [limit] FROM t /* offset */"),
        ];
        for (dialect, sql) in rows {
            let limited = limit_select(sql, dialect, 7).unwrap_or_default();
            assert!(limited.contains(" LIMIT 7"), "{dialect:?}: {limited}");
        }
    }

    #[test]
    fn writing_ctes_and_mysql_locks_are_left_alone() {
        let pg = "WITH gone AS (DELETE FROM t RETURNING *) SELECT * FROM gone";
        assert_eq!(limit_select(pg, Dialect::Postgres, 5), None);
        let lock = "SELECT * FROM t LOCK IN SHARE MODE";
        assert_eq!(limit_select(lock, Dialect::MySql, 5), None);
    }
}
