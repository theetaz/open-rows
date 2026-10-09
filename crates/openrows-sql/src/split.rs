// SPDX-License-Identifier: Apache-2.0
//! Statement splitting and current-statement detection.

mod blocks;
mod delimiter;
mod stream;

use std::ops::Range;

use crate::Dialect;
use crate::lexer::{Lexer, Token, TokenKind};
use blocks::Blocks;
use delimiter::Delimiter;
pub use stream::{ScriptStatement, StreamSplitter};

/// One statement found by [`split`], located by byte offsets in the original text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Statement {
    /// Bytes from the first non-whitespace token (leading comments included)
    /// to the end of the last non-whitespace token, excluding the terminator.
    pub range: Range<usize>,
    /// Byte range of the terminator: `;`, or the custom delimiter set by a
    /// `MySQL` `DELIMITER` command. `None` for a trailing statement without one.
    pub terminator: Option<Range<usize>>,
}

impl Statement {
    /// The statement text (without the terminator) sliced from `source`.
    ///
    /// Returns an empty string if `source` is not the text that was split.
    #[must_use]
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        source.get(self.range.clone()).unwrap_or_default()
    }

    /// The statement range extended to include its terminator, if any. Use
    /// this to highlight exactly what the user sees as "the statement".
    #[must_use]
    pub fn range_with_terminator(&self) -> Range<usize> {
        let end = self
            .terminator
            .as_ref()
            .map_or(self.range.end, |terminator| terminator.end);
        self.range.start..end
    }
}

/// Split `source` into statements. Never fails and never panics.
///
/// A `;` terminates a statement only outside strings, quoted identifiers,
/// comments, parentheses and routine bodies: `PostgreSQL` `BEGIN ATOMIC … END`
/// (and dollar quotes, as in `psql`), `SQLite` `CREATE TRIGGER … BEGIN … END`,
/// and `MySQL` `CREATE PROCEDURE|FUNCTION|TRIGGER|EVENT … BEGIN … END`. For
/// `MySQL`, a `DELIMITER <text>` line (as in the `mysql` client) replaces the
/// terminator until the next `DELIMITER` line; the line itself is no
/// statement. Statements consisting only of whitespace and comments
/// (including empty `;;`) are dropped, and a comment on the same line after a
/// terminator belongs to no statement. Unterminated strings or comments
/// extend to the end of the text.
#[must_use]
pub fn split(source: &str, dialect: Dialect) -> Vec<Statement> {
    split_resumed(source, dialect, Resume::default()).0
}

/// Splitter state at a statement boundary, which text after it resumes from.
#[derive(Clone, Debug, Default)]
struct Resume {
    delimiter: Delimiter,
    /// Just after a terminator: a comment on the same line belongs to no statement.
    same_line: bool,
}

/// [`split`] starting in `state`; also returns the last boundary (the byte
/// offset just after a terminator) with the state there.
fn split_resumed(
    source: &str,
    dialect: Dialect,
    state: Resume,
) -> (Vec<Statement>, Option<(usize, Resume)>) {
    let mut builder = Builder::new(dialect, state);
    let mut lexer = Lexer::new(source, dialect);
    while let Some(token) = lexer.next() {
        if let Some(resume) = builder.push(token, source) {
            lexer = Lexer::resume(source, resume, dialect);
        }
    }
    builder.finish(None);
    (builder.statements, builder.boundary)
}

struct Builder {
    dialect: Dialect,
    start: Option<usize>,
    end: usize,
    has_code: bool,
    paren_depth: usize,
    blocks: Blocks,
    delimiter: Delimiter,
    /// Still on the line of the previous terminator: comments here belong to no statement.
    same_line: bool,
    statements: Vec<Statement>,
    /// The last terminator's end and the state after it.
    boundary: Option<(usize, Resume)>,
}

impl Builder {
    fn new(dialect: Dialect, state: Resume) -> Self {
        Self {
            dialect,
            start: None,
            end: 0,
            has_code: false,
            paren_depth: 0,
            blocks: Blocks::new(dialect),
            delimiter: state.delimiter,
            same_line: state.same_line,
            statements: Vec::new(),
            boundary: None,
        }
    }

    /// Feed one token; returns a byte offset to resume lexing from when the
    /// token started a `DELIMITER` line or ended at a custom delimiter.
    fn push(&mut self, token: Token, source: &str) -> Option<usize> {
        match token.kind {
            TokenKind::Whitespace => {
                self.same_line &= !token.text(source).contains('\n');
                return None;
            }
            TokenKind::LineComment | TokenKind::BlockComment if self.same_line => return None,
            _ => {}
        }
        if let Some(resume) = self.delimiter_command(token, source) {
            return Some(resume);
        }
        if let Some(found) = self.delimiter.found_at(token, source) {
            if found.start > token.start {
                let before = Token {
                    end: found.start,
                    ..token
                };
                self.extend(before, source);
            }
            self.finish(Some(found.clone()));
            return Some(found.end);
        }
        let terminates = token.kind == TokenKind::Semicolon
            && self.delimiter.is_default()
            && self.paren_depth == 0
            && self.blocks.is_closed();
        if terminates {
            self.finish(Some(token.start..token.end));
            return None;
        }
        self.extend(token, source);
        None
    }

    /// A `MySQL` `DELIMITER` line at the start of a statement.
    fn delimiter_command(&mut self, token: Token, source: &str) -> Option<usize> {
        let starts = self.dialect == Dialect::MySql && !self.has_code;
        if !(starts && token.is_word(source, "DELIMITER")) {
            return None;
        }
        let line_end = self.delimiter.set_from_line(source, token.end);
        self.start = None;
        self.same_line = false;
        Some(line_end)
    }

    fn extend(&mut self, token: Token, source: &str) {
        self.same_line = false;
        self.start.get_or_insert(token.start);
        self.end = token.end;
        if token.kind.is_trivia() {
            return;
        }
        self.has_code = true;
        match token.kind {
            TokenKind::OpenParen => self.paren_depth += 1,
            TokenKind::CloseParen => self.paren_depth = self.paren_depth.saturating_sub(1),
            _ => {}
        }
        self.blocks.track(token, source);
    }

    fn finish(&mut self, terminator: Option<Range<usize>>) {
        let ended = terminator.is_some();
        if let Some(terminator) = &terminator {
            let state = Resume {
                delimiter: self.delimiter.clone(),
                same_line: true,
            };
            self.boundary = Some((terminator.end, state));
        }
        if let (Some(start), true) = (self.start, self.has_code) {
            self.statements.push(Statement {
                range: start..self.end,
                terminator,
            });
        }
        self.start = None;
        self.has_code = false;
        self.paren_depth = 0;
        self.blocks = Blocks::new(self.dialect);
        self.same_line = ended;
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
