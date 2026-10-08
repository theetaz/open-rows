// SPDX-License-Identifier: Apache-2.0
//! Token-based safety classification of a single statement.
//!
//! The classifier works on lexer tokens, never on a full parse, so it handles
//! incomplete SQL and cannot fail. Whenever it cannot prove a statement is
//! harmless it answers [`StatementKind::Unknown`], which callers treat as a
//! write.

mod heads;
mod mysql;
mod mysql_set;
mod scan;
mod session;
mod sqlite;

use crate::Dialect;
use crate::lexer::{self, Token, TokenKind};

/// What a statement does, for safe-mode decisions.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum StatementKind {
    /// Reads data or session state only (`SELECT`, `SHOW`, `EXPLAIN`, …).
    Read,
    /// Changes data or server state (`INSERT`, `UPDATE`, `DELETE`, `COPY … FROM`, …).
    Write,
    /// Changes schema or privileges (`CREATE`, `ALTER`, `DROP`, `GRANT`, …).
    Ddl,
    /// Transaction control (`BEGIN`, `COMMIT`, `ROLLBACK`, `SAVEPOINT`, …).
    TxControl,
    /// Not recognised; must be treated as a write.
    Unknown,
}

impl StatementKind {
    /// Whether safe mode must treat this kind as modifying the database.
    /// `Unknown` counts as a write.
    #[must_use]
    pub fn is_write(self) -> bool {
        matches!(self, Self::Write | Self::Ddl | Self::Unknown)
    }
}

/// Result of [`classify`].
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Classification {
    /// What the statement does.
    pub kind: StatementKind,
    /// Requires explicit confirmation: `DELETE`/`UPDATE` without `WHERE`
    /// (including inside CTEs), `DROP`, `TRUNCATE`, `ALTER … DROP` and, for
    /// `MySQL`, `SHUTDOWN`.
    pub dangerous: bool,
}

impl Classification {
    const fn new(kind: StatementKind) -> Self {
        Self {
            kind,
            dangerous: false,
        }
    }

    const fn dangerous(kind: StatementKind) -> Self {
        Self {
            kind,
            dangerous: true,
        }
    }

    /// Shorthand for `self.kind.is_write()`.
    #[must_use]
    pub fn is_write(self) -> bool {
        self.kind.is_write()
    }
}

/// Classify one statement (typically [`crate::Statement::text`]).
///
/// Leading comments, whitespace and parentheses are skipped. Empty input is
/// `Unknown`. Text containing a `;` the splitter kept inside the statement
/// (for example after an unbalanced `(`) is never classified as `Read`. The
/// body of a `MySQL` `/*! … */` executable comment is classified as code,
/// because the server runs it.
#[must_use]
pub fn classify(statement: &str, dialect: Dialect) -> Classification {
    let tokens = lexer::significant(statement, dialect);
    Sql::new(statement, &tokens, dialect).classify()
}

/// A slice of significant tokens with the source they index into.
#[derive(Clone, Copy)]
struct Sql<'a> {
    source: &'a str,
    tokens: &'a [Token],
    dialect: Dialect,
}

impl<'a> Sql<'a> {
    fn new(source: &'a str, tokens: &'a [Token], dialect: Dialect) -> Self {
        Self {
            source,
            tokens,
            dialect,
        }
    }

    fn classify(self) -> Classification {
        let body = self.from(self.leading_parens());
        let mut result = match (self.dialect, body.is_word(0, "EXPLAIN")) {
            (Dialect::Postgres, true) => heads::explain(body),
            (Dialect::MySql, true) => mysql::explain(body),
            (Dialect::Sqlite, true) => Classification::new(StatementKind::Read),
            (Dialect::Postgres, false) => body.inspect(heads::dispatch(body)),
            (Dialect::MySql, false) => body.inspect(mysql::dispatch(body)),
            (Dialect::Sqlite, false) => body.inspect(sqlite::dispatch(body)),
        };
        if body.has_kind(TokenKind::Semicolon) {
            result.dangerous |= scan::has_unbounded_dml(body);
            if result.kind == StatementKind::Read {
                result.kind = StatementKind::Unknown;
            }
        }
        result
    }

    /// Escalate a `Read` that hides writes and flag unbounded `DELETE`/`UPDATE`.
    fn inspect(self, base: Classification) -> Classification {
        let kind = match base.kind {
            StatementKind::Read if scan::has_write_marker(self) => StatementKind::Write,
            kind => kind,
        };
        Classification {
            kind,
            dangerous: base.dangerous || scan::has_unbounded_dml(self),
        }
    }

    fn leading_parens(self) -> usize {
        self.tokens
            .iter()
            .take_while(|token| token.kind == TokenKind::OpenParen)
            .count()
    }

    fn from(self, index: usize) -> Self {
        Self::new(
            self.source,
            self.tokens.get(index..).unwrap_or_default(),
            self.dialect,
        )
    }

    fn kind(self, index: usize) -> Option<TokenKind> {
        self.tokens.get(index).map(|token| token.kind)
    }

    fn has_kind(self, kind: TokenKind) -> bool {
        self.tokens.iter().any(|token| token.kind == kind)
    }

    fn is_word(self, index: usize, keyword: &str) -> bool {
        self.tokens
            .get(index)
            .is_some_and(|token| token.is_word(self.source, keyword))
    }

    fn has_word(self, keyword: &str) -> bool {
        self.tokens
            .iter()
            .any(|token| token.is_word(self.source, keyword))
    }

    /// Upper-cased unquoted word at `index`, if the token is a word.
    fn word(self, index: usize) -> Option<String> {
        let token = self.tokens.get(index)?;
        (token.kind == TokenKind::Word).then(|| token.text(self.source).to_ascii_uppercase())
    }

    fn text(self, index: usize) -> &'a str {
        self.tokens
            .get(index)
            .map_or("", |token| token.text(self.source))
    }

    /// First index at or after `from` holding one of `keywords` outside parentheses.
    fn find_top_level(self, from: usize, keywords: &[&str]) -> Option<usize> {
        let mut depth = 0_usize;
        for (index, token) in self.tokens.iter().enumerate().skip(from) {
            match token.kind {
                TokenKind::OpenParen => depth += 1,
                TokenKind::CloseParen => depth = depth.saturating_sub(1),
                TokenKind::Word
                    if depth == 0
                        && keywords
                            .iter()
                            .any(|keyword| token.is_word(self.source, keyword)) =>
                {
                    return Some(index);
                }
                _ => {}
            }
        }
        None
    }

    /// Whether the words `first second` appear adjacently anywhere.
    fn has_word_pair(self, first: &str, second: &str) -> bool {
        self.tokens.windows(2).any(|pair| match pair {
            [left, right] => left.is_word(self.source, first) && right.is_word(self.source, second),
            _ => false,
        })
    }
}
