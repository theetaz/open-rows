// SPDX-License-Identifier: Apache-2.0
//! Statement splitting and current-statement detection.

use std::ops::Range;

use crate::Dialect;
use crate::lexer::{Lexer, Token, TokenKind};

/// One statement found by [`split`], located by byte offsets in the original text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Statement {
    /// Bytes from the first non-whitespace token (leading comments included)
    /// to the end of the last non-whitespace token, excluding the `;`.
    pub range: Range<usize>,
    /// Byte offset of the terminating `;`, or `None` for a trailing
    /// statement without one.
    pub terminator: Option<usize>,
}

impl Statement {
    /// The statement text (without the terminator) sliced from `source`.
    ///
    /// Returns an empty string if `source` is not the text that was split.
    #[must_use]
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        source.get(self.range.clone()).unwrap_or_default()
    }

    /// The statement range extended to include its `;`, if any. Use this to
    /// highlight exactly what the user sees as "the statement".
    #[must_use]
    pub fn range_with_terminator(&self) -> Range<usize> {
        let end = self.terminator.map_or(self.range.end, |offset| offset + 1);
        self.range.start..end
    }
}

/// Split `source` into statements. Never fails and never panics.
///
/// A `;` terminates a statement only outside strings, quoted identifiers,
/// dollar quotes, comments, parentheses and SQL-standard `BEGIN ATOMIC … END`
/// function bodies (the same rules as `psql`). Statements consisting only of
/// whitespace and comments (including empty `;;`) are dropped, and a comment
/// on the same line after a `;` belongs to no statement. Unterminated
/// strings or comments extend to the end of the text.
#[must_use]
pub fn split(source: &str, dialect: Dialect) -> Vec<Statement> {
    match dialect {
        Dialect::Postgres => split_postgres(source),
    }
}

fn split_postgres(source: &str) -> Vec<Statement> {
    let mut builder = Builder::default();
    for token in Lexer::new(source) {
        builder.push(token, source);
    }
    builder.finish(None);
    builder.statements
}

#[derive(Default)]
struct Builder {
    start: Option<usize>,
    end: usize,
    has_code: bool,
    paren_depth: usize,
    atomic_depth: usize,
    after_begin: bool,
    /// Still on the line of the previous `;`: comments here belong to no statement.
    same_line: bool,
    statements: Vec<Statement>,
}

impl Builder {
    fn push(&mut self, token: Token, source: &str) {
        match token.kind {
            TokenKind::Whitespace => {
                self.same_line &= !token.text(source).contains('\n');
                return;
            }
            TokenKind::LineComment | TokenKind::BlockComment if self.same_line => return,
            TokenKind::Semicolon if self.paren_depth == 0 && self.atomic_depth == 0 => {
                self.finish(Some(token.start));
                return;
            }
            _ => {}
        }
        self.same_line = false;
        self.start.get_or_insert(token.start);
        self.end = token.end;
        if !token.kind.is_trivia() {
            self.has_code = true;
            self.track_nesting(token, source);
        }
    }

    fn track_nesting(&mut self, token: Token, source: &str) {
        match token.kind {
            TokenKind::OpenParen => self.paren_depth += 1,
            TokenKind::CloseParen => self.paren_depth = self.paren_depth.saturating_sub(1),
            TokenKind::Word => self.track_atomic(token, source),
            _ => {}
        }
        self.after_begin = token.is_word(source, "BEGIN");
    }

    /// `BEGIN ATOMIC` opens a block closed by `END`; `CASE … END` nests inside it.
    fn track_atomic(&mut self, token: Token, source: &str) {
        let opens_atomic = self.after_begin && token.is_word(source, "ATOMIC");
        let opens_case = self.atomic_depth > 0 && token.is_word(source, "CASE");
        if opens_atomic || opens_case {
            self.atomic_depth += 1;
        } else if self.atomic_depth > 0 && token.is_word(source, "END") {
            self.atomic_depth -= 1;
        }
    }

    fn finish(&mut self, terminator: Option<usize>) {
        if let (Some(start), true) = (self.start, self.has_code) {
            self.statements.push(Statement {
                range: start..self.end,
                terminator,
            });
        }
        self.start = None;
        self.has_code = false;
        self.after_begin = false;
        self.same_line = terminator.is_some();
    }
}

/// Index of the statement to run for a cursor at byte offset `cursor`.
///
/// Returns the statement containing the cursor. Between statements (after a
/// `;`, on blank lines, or on a comment trailing the `;` on its line) the
/// closest preceding statement wins, so pressing ⌘↵ just after typing `…;`
/// runs what was typed. A cursor exactly at a statement's first byte selects
/// that statement. Before the first statement, the first statement is
/// returned. `None` only when `statements` is empty.
///
/// `statements` must be ordered as returned by [`split`].
#[must_use]
pub fn statement_at(statements: &[Statement], cursor: usize) -> Option<usize> {
    if statements.is_empty() {
        return None;
    }
    let after = statements.partition_point(|statement| statement.range.start <= cursor);
    Some(after.saturating_sub(1))
}
