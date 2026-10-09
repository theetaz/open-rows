// SPDX-License-Identifier: Apache-2.0
//! Raw SQL filter predicates (FR-GRID-11, docs/08 §10).
//!
//! A raw predicate is user SQL, so it is checked lexically before it is
//! spliced into the page `SELECT`: it must stay one parenthesised boolean
//! expression. Comments (which could hide the closing parenthesis or the
//! `LIMIT`), statement terminators, bind placeholders (which would shift
//! the generated parameter numbers) and parentheses that close the wrapper
//! are refused. The caller still classifies the whole statement, so a raw
//! predicate passes the same read-only safety check as any table page.

use super::SelectError;
use crate::lexer::{Lexer, Token, TokenKind};
use crate::{Dialect, PlaceholderKind, placeholders};

/// Text the predicate is lexed after, so tokens see a real `WHERE` context.
const PREFIX: &str = "SELECT 1 WHERE (";

/// The validated predicate, wrapped in parentheses.
pub(super) fn predicate(dialect: Dialect, text: &str) -> Result<String, SelectError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(SelectError::InvalidPredicate("is empty"));
    }
    if trimmed.contains('\0') {
        return Err(SelectError::NulCharacter);
    }
    check(dialect, trimmed)?;
    if binds(dialect, trimmed) {
        return Err(SelectError::InvalidPredicate(
            "cannot contain bind parameters; type the values instead",
        ));
    }
    Ok(format!("({trimmed})"))
}

/// Placeholders would shift the generated parameter numbers; `PostgreSQL`
/// reads a bare `?` as a `jsonb` operator, so it stays allowed there.
fn binds(dialect: Dialect, text: &str) -> bool {
    placeholders(text, dialect)
        .iter()
        .any(|found| !(dialect == Dialect::Postgres && found.kind == PlaceholderKind::Anonymous))
}

/// Lex `PREFIX + text + ")"` and require the final `)` to close the wrapper.
fn check(dialect: Dialect, text: &str) -> Result<(), SelectError> {
    let probe = format!("{PREFIX}{text})");
    let mut depth = 0_usize;
    for token in Lexer::new(&probe, dialect).filter(|token| token.start >= PREFIX.len()) {
        if token.end == probe.len() {
            return closing(token, depth);
        }
        depth = step(token.kind, depth)?;
    }
    Err(SelectError::InvalidPredicate(
        "has an unterminated string or quoted name",
    ))
}

/// The wrapper's own `)`: it must be a real token at depth zero.
fn closing(token: Token, depth: usize) -> Result<(), SelectError> {
    match (token.kind, depth) {
        (TokenKind::CloseParen, 0) => Ok(()),
        (TokenKind::CloseParen, _) => {
            Err(SelectError::InvalidPredicate("has unbalanced parentheses"))
        }
        (TokenKind::LineComment | TokenKind::BlockComment, _) => {
            Err(SelectError::InvalidPredicate("cannot contain comments"))
        }
        _ => Err(SelectError::InvalidPredicate(
            "has an unterminated string or quoted name",
        )),
    }
}

/// Parenthesis depth after one token of the predicate.
fn step(kind: TokenKind, depth: usize) -> Result<usize, SelectError> {
    match kind {
        TokenKind::LineComment | TokenKind::BlockComment => {
            Err(SelectError::InvalidPredicate("cannot contain comments"))
        }
        TokenKind::Semicolon => Err(SelectError::InvalidPredicate(
            "must be one expression without `;`",
        )),
        TokenKind::Param => Err(SelectError::InvalidPredicate(
            "cannot contain bind parameters; type the values instead",
        )),
        TokenKind::OpenParen => Ok(depth + 1),
        TokenKind::CloseParen => depth
            .checked_sub(1)
            .ok_or(SelectError::InvalidPredicate("has unbalanced parentheses")),
        _ => Ok(depth),
    }
}
