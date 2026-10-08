// SPDX-License-Identifier: Apache-2.0
//! Routine bodies whose inner `;` do not end the statement:
//! `PostgreSQL` `BEGIN ATOMIC … END`, `SQLite` `CREATE TRIGGER … BEGIN … END`
//! and `MySQL` compound statements in `CREATE PROCEDURE|FUNCTION|TRIGGER|EVENT`
//! (and `MariaDB` `BEGIN NOT ATOMIC`). `CASE … END` nests inside a body.

use crate::Dialect;
use crate::lexer::{Token, TokenKind};

/// Words that make a `MySQL` `CREATE` statement a compound-statement routine.
const MYSQL_ROUTINES: &[&str] = &["PROCEDURE", "FUNCTION", "TRIGGER", "EVENT"];

/// Words after `END` that close a `MySQL` construct this tracker does not count.
const MYSQL_UNCOUNTED: &[&str] = &["IF", "LOOP", "WHILE", "REPEAT"];

/// Block nesting of the statement being split.
#[derive(Debug)]
pub(super) struct Blocks {
    dialect: Dialect,
    depth: usize,
    /// Significant words seen so far in the statement.
    words: usize,
    /// The statement starts with `CREATE` (or `MySQL` `BEGIN`).
    head: Option<Head>,
    /// `BEGIN` opens a body in this statement.
    routine: bool,
    /// The previous significant token was `BEGIN`.
    after_begin: bool,
    /// `MySQL`: an `END` whose meaning depends on the next word.
    pending_end: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Head {
    Create,
    Begin,
}

impl Blocks {
    pub(super) fn new(dialect: Dialect) -> Self {
        Self {
            dialect,
            depth: 0,
            words: 0,
            head: None,
            routine: false,
            after_begin: false,
            pending_end: false,
        }
    }

    /// No body is open, so a `;` ends the statement.
    pub(super) fn is_closed(&self) -> bool {
        self.depth.saturating_sub(usize::from(self.pending_end)) == 0
    }

    /// Track one significant token.
    pub(super) fn track(&mut self, token: Token, source: &str) {
        let closed_case = self.pending_end && self.resolve_end(token, source);
        if token.kind == TokenKind::Word {
            if !closed_case {
                self.word(token, source);
            }
            self.words += 1;
        }
        self.after_begin = token.is_word(source, "BEGIN");
    }

    fn word(&mut self, token: Token, source: &str) {
        let is = |keyword: &str| token.is_word(source, keyword);
        if self.words == 0 {
            self.head = if is("CREATE") {
                Some(Head::Create)
            } else if is("BEGIN") {
                Some(Head::Begin)
            } else {
                None
            };
        } else {
            self.detect_routine(token, source);
        }
        if self.opens(token, source) {
            self.depth += 1;
        } else if self.depth > 0 && is("END") {
            if self.dialect == Dialect::MySql {
                self.pending_end = true;
            } else {
                self.depth -= 1;
            }
        }
    }

    fn detect_routine(&mut self, token: Token, source: &str) {
        let is = |keyword: &str| token.is_word(source, keyword);
        self.routine |= match (self.dialect, self.head) {
            (Dialect::Sqlite, Some(Head::Create)) => is("TRIGGER"),
            (Dialect::MySql, Some(Head::Create)) => MYSQL_ROUTINES.iter().any(|word| is(word)),
            // `MariaDB` anonymous block: `BEGIN NOT ATOMIC … END`.
            (Dialect::MySql, Some(Head::Begin)) => self.words == 1 && is("NOT"),
            _ => false,
        };
        if self.dialect == Dialect::MySql && self.head == Some(Head::Begin) && is("ATOMIC") {
            self.depth += usize::from(self.routine && self.depth == 0);
        }
    }

    fn opens(&self, token: Token, source: &str) -> bool {
        let is = |keyword: &str| token.is_word(source, keyword);
        match self.dialect {
            Dialect::Postgres => {
                (self.after_begin && is("ATOMIC")) || (self.depth > 0 && is("CASE"))
            }
            Dialect::Sqlite | Dialect::MySql => {
                (self.routine && is("BEGIN")) || (self.depth > 0 && is("CASE"))
            }
        }
    }

    /// `END IF|LOOP|WHILE|REPEAT` closes nothing counted; `END CASE` closes
    /// the counted `CASE`; any other `END` closes the innermost block.
    /// Returns true when `token` is the `CASE` of `END CASE` (no new `CASE`).
    fn resolve_end(&mut self, token: Token, source: &str) -> bool {
        self.pending_end = false;
        let uncounted = MYSQL_UNCOUNTED
            .iter()
            .any(|word| token.is_word(source, word));
        if !uncounted {
            self.depth = self.depth.saturating_sub(1);
        }
        token.is_word(source, "CASE")
    }
}
