// SPDX-License-Identifier: Apache-2.0
//! SQL Server's automatic row limit: `SELECT TOP (n) …`. T-SQL has no
//! `LIMIT`, and `OFFSET … FETCH` needs an `ORDER BY`, so `TOP (n)` goes
//! after the main `SELECT` (and its `ALL`/`DISTINCT`). Only a single plain
//! query is rewritten: not one already bounded (`TOP`, `OFFSET`, `FETCH`,
//! `INTO`, `FOR`), not a `UNION`/`INTERSECT`/`EXCEPT` (`TOP` would bind to
//! the first branch only), not a batch holding a second top-level `SELECT`,
//! and not a variable assignment (`SELECT @v = …`).
use super::is_bounded;
use crate::lexer::{Lexer, Token, TokenKind};
use crate::{Dialect, StatementKind, classify};

const SET_OPERATORS: &[&str] = &["UNION", "INTERSECT", "EXCEPT"];

/// `statement` with `TOP (rows)` after its main `SELECT`, or `None`.
pub(super) fn limit(statement: &str, rows: u64) -> Option<String> {
    let tokens: Vec<Token> = Lexer::new(statement, Dialect::SqlServer)
        .filter(|token| !token.kind.is_trivia())
        .collect();
    let first = tokens.first()?;
    let reads = first.is_word(statement, "SELECT")
        || (first.is_word(statement, "WITH")
            && classify(statement, Dialect::SqlServer).kind == StatementKind::Read);
    if !reads || is_bounded(statement, &tokens) {
        return None;
    }
    let selects = top_level(statement, &tokens, "SELECT");
    let [main] = selects.as_slice() else {
        return None;
    };
    if SET_OPERATORS
        .iter()
        .any(|word| !top_level(statement, &tokens, word).is_empty())
    {
        return None;
    }
    let mut after = *main + 1;
    if tokens.get(after).is_some_and(|token| {
        token.is_word(statement, "ALL") || token.is_word(statement, "DISTINCT")
    }) {
        after += 1;
    }
    let next = tokens.get(after)?;
    if next.text(statement).starts_with('@') {
        return None;
    }
    let at = tokens.get(after - 1)?.end;
    let (head, tail) = statement.split_at(at);
    Some(format!("{head} TOP ({rows}){tail}"))
}

/// Indexes of `word` outside parentheses.
fn top_level(statement: &str, tokens: &[Token], word: &str) -> Vec<usize> {
    let mut depth = 0_usize;
    let mut found = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        match token.kind {
            TokenKind::OpenParen => depth += 1,
            TokenKind::CloseParen => depth = depth.saturating_sub(1),
            TokenKind::Word if depth == 0 && token.is_word(statement, word) => found.push(index),
            _ => {}
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::limit;

    #[test]
    fn plain_selects_get_top_after_select_and_distinct() {
        let cases = [
            ("SELECT * FROM t", "SELECT TOP (50) * FROM t"),
            (
                "select distinct a from t -- note",
                "select distinct TOP (50) a from t -- note",
            ),
            (
                "SELECT a FROM t WHERE b IN (SELECT b FROM u) ORDER BY a",
                "SELECT TOP (50) a FROM t WHERE b IN (SELECT b FROM u) ORDER BY a",
            ),
            (
                "WITH c AS (SELECT 1 AS a) SELECT a FROM c",
                "WITH c AS (SELECT 1 AS a) SELECT TOP (50) a FROM c",
            ),
        ];
        for (sql, expected) in cases {
            assert_eq!(limit(sql, 50).as_deref(), Some(expected), "{sql}");
        }
    }

    #[test]
    fn bounded_combined_or_assigning_selects_run_as_written() {
        for sql in [
            "SELECT TOP 5 * FROM t",
            "SELECT * FROM t ORDER BY a OFFSET 0 ROWS FETCH NEXT 5 ROWS ONLY",
            "SELECT * INTO #copy FROM t",
            "SELECT a FROM t UNION ALL SELECT a FROM u",
            "SELECT 1 SELECT 2",
            "SELECT @v = a FROM t",
            "DELETE FROM t",
            "WITH c AS (SELECT 1 AS a) DELETE FROM t",
            "SELECT 1;",
        ] {
            assert_eq!(limit(sql, 50), None, "{sql}");
        }
    }
}
