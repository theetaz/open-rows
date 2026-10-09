// SPDX-License-Identifier: Apache-2.0
//! Incremental splitting of scripts too large to hold in memory (FR-IO-03).
//!
//! Text arrives in chunks; only the unfinished tail after the last statement
//! terminator is kept. A terminator is final once seen, because the lexer
//! reads forward only: no later text changes how earlier text splits. The
//! tail is split again when more text arrives, resuming with the state at
//! the last terminator (a `MySQL` `DELIMITER` in force, the trailing-comment
//! rule), so the statements are exactly those [`split`](super::split) finds
//! on the whole text.
use super::{Resume, split_resumed};
use crate::Dialect;
use crate::lexer::Lexer;

/// Bytes a tail must reach before it is split again; doubles after every
/// split that found no terminator, so a huge statement costs linear time.
const MIN_SCAN: usize = 64 * 1024;

/// One statement of a streamed script, located in the whole script.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScriptStatement {
    /// The statement text without its terminator.
    pub text: String,
    /// 1-based line of the statement's first code, after leading comments.
    pub line: usize,
    /// Byte offset of the statement's first byte.
    pub offset: u64,
    /// Byte offset just after the statement (its terminator included).
    pub end: u64,
}

/// Splits a script fed in chunks; see the module docs.
#[derive(Debug)]
pub struct StreamSplitter {
    dialect: Dialect,
    /// Text after the last terminator.
    tail: String,
    state: Resume,
    /// Bytes and line breaks before `tail`.
    consumed: u64,
    lines: usize,
    /// Split `tail` again once it holds this many bytes.
    scan_at: usize,
}

impl StreamSplitter {
    /// A splitter for a script in `dialect`.
    #[must_use]
    pub fn new(dialect: Dialect) -> Self {
        Self {
            dialect,
            tail: String::new(),
            state: Resume::default(),
            consumed: 0,
            lines: 0,
            scan_at: 0,
        }
    }

    /// Append the next chunk; returns the statements it completed.
    pub fn push(&mut self, chunk: &str) -> Vec<ScriptStatement> {
        self.tail.push_str(chunk);
        if self.tail.len() < self.scan_at {
            return Vec::new();
        }
        let (statements, boundary) = split_resumed(&self.tail, self.dialect, self.state.clone());
        let boundary = boundary.filter(|(end, _)| self.tail.is_char_boundary(*end));
        let Some((end, state)) = boundary else {
            self.scan_at = (self.tail.len() * 2).max(MIN_SCAN);
            return Vec::new();
        };
        let complete = statements
            .into_iter()
            .filter(|statement| statement.range.end <= end);
        let found = self.locate(complete);
        self.advance(end);
        self.state = state;
        self.scan_at = 0;
        found
    }

    /// The statements of the remaining text, terminated or not.
    pub fn finish(&mut self) -> Vec<ScriptStatement> {
        let (statements, _) = split_resumed(&self.tail, self.dialect, self.state.clone());
        let found = self.locate(statements.into_iter());
        let end = self.tail.len();
        self.advance(end);
        found
    }

    /// Bytes of the script consumed into returned statements (or skipped).
    #[must_use]
    pub fn consumed(&self) -> u64 {
        self.consumed
    }

    /// Statements of `tail` with their text, line and offsets.
    fn locate(&self, statements: impl Iterator<Item = super::Statement>) -> Vec<ScriptStatement> {
        let mut line = self.lines + 1;
        let mut counted = 0;
        statements
            .map(|statement| {
                let start = statement.range.start;
                line += newlines(self.tail.get(counted..start).unwrap_or_default());
                counted = start;
                let end = statement.range_with_terminator().end;
                let text = statement.text(&self.tail);
                let code = Lexer::new(text, self.dialect)
                    .find(|token| !token.kind.is_trivia())
                    .map_or(0, |token| token.start);
                ScriptStatement {
                    text: text.to_owned(),
                    line: line + newlines(text.get(..code).unwrap_or_default()),
                    offset: self.consumed + start as u64,
                    end: self.consumed + end as u64,
                }
            })
            .collect()
    }

    /// Drop the first `end` bytes of `tail`.
    fn advance(&mut self, end: usize) {
        self.lines += newlines(self.tail.get(..end).unwrap_or_default());
        self.consumed += end as u64;
        self.tail.drain(..end);
    }
}

fn newlines(text: &str) -> usize {
    text.bytes().filter(|byte| *byte == b'\n').count()
}
