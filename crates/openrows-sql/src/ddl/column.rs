// SPDX-License-Identifier: Apache-2.0
//! One column definition (`name type constraint…`) whose type, nullability,
//! default and comment can be replaced while every other clause
//! (`PRIMARY KEY`, `AUTO_INCREMENT`, `ON UPDATE …`, `CHARACTER SET`,
//! `REFERENCES …`, `GENERATED …`) is kept as written.
use crate::Dialect;
use crate::lexer::{Lexer, Token, TokenKind};

/// Words that start a column constraint clause after the type.
const CLAUSE_WORDS: [&str; 18] = [
    "CONSTRAINT",
    "PRIMARY",
    "NOT",
    "NULL",
    "UNIQUE",
    "CHECK",
    "DEFAULT",
    "COLLATE",
    "REFERENCES",
    "GENERATED",
    "AS",
    "AUTO_INCREMENT",
    "COMMENT",
    "ON",
    "VISIBLE",
    "INVISIBLE",
    "STORAGE",
    "COLUMN_FORMAT",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Clause {
    NotNull,
    Null,
    Default,
    Collate,
    Comment,
    Generated,
    Other,
}

/// A column definition split into name, type and constraint clauses.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ColumnDefinition {
    name: String,
    data_type: String,
    clauses: Vec<(Clause, String)>,
}

impl ColumnDefinition {
    /// Split one column definition; `None` when it has no name.
    #[must_use]
    pub fn parse(text: &str, dialect: Dialect) -> Option<Self> {
        let tokens: Vec<Token> = Lexer::new(text, dialect)
            .filter(|token| !matches!(token.kind, TokenKind::LineComment | TokenKind::BlockComment))
            .collect();
        let name = tokens.iter().find(|token| !token.kind.is_trivia())?;
        let mut parts = Parts::default();
        for token in tokens.iter().skip_while(|token| token.start <= name.start) {
            parts.push(text, *token);
        }
        Some(Self {
            name: name.text(text).to_owned(),
            data_type: parts.data_type.trim().to_owned(),
            clauses: parts.finish(),
        })
    }

    /// A new column: `name type [NOT NULL] [DEFAULT default]`.
    #[must_use]
    pub fn new(quoted_name: &str, data_type: &str) -> Self {
        Self {
            name: quoted_name.to_owned(),
            data_type: data_type.to_owned(),
            clauses: Vec::new(),
        }
    }

    /// Replace the (quoted) name.
    pub fn set_name(&mut self, quoted_name: &str) {
        quoted_name.clone_into(&mut self.name);
    }

    /// Replace the type; its old collation goes with it.
    pub fn set_type(&mut self, data_type: &str) {
        data_type.clone_into(&mut self.data_type);
        self.clauses
            .retain(|(clause, _)| *clause != Clause::Collate);
    }

    /// `NOT NULL` where the old nullability clause was (else after the
    /// collation and generation clauses), or no nullability clause at all.
    pub fn set_nullable(&mut self, nullable: bool) {
        let at = self.remove(&[Clause::NotNull, Clause::Null]);
        if !nullable {
            let at = at.unwrap_or_else(|| self.after(&[Clause::Collate, Clause::Generated]));
            self.clauses
                .insert(at, (Clause::NotNull, "NOT NULL".to_owned()));
        }
    }

    /// `DEFAULT sql` where the old default was (else after the nullability
    /// clause), or no default.
    pub fn set_default(&mut self, sql: Option<&str>) {
        let at = self.remove(&[Clause::Default]);
        if let Some(sql) = sql {
            let at = at.unwrap_or_else(|| {
                self.after(&[
                    Clause::Collate,
                    Clause::Generated,
                    Clause::NotNull,
                    Clause::Null,
                ])
            });
            self.clauses
                .insert(at, (Clause::Default, format!("DEFAULT {sql}")));
        }
    }

    /// `COMMENT literal` at the end (`MySQL`), or no comment.
    pub fn set_comment(&mut self, literal: Option<&str>) {
        self.clauses
            .retain(|(clause, _)| *clause != Clause::Comment);
        if let Some(literal) = literal {
            self.clauses
                .push((Clause::Comment, format!("COMMENT {literal}")));
        }
    }

    /// Replace the character set (kept in the type text, as `MySQL` prints
    /// it) and the collation; `None` leaves each to the table's default.
    pub fn set_collation(&mut self, charset: Option<&str>, collation: Option<&str>) {
        let words: Vec<&str> = self.data_type.split_whitespace().collect();
        let mut kept: Vec<&str> = Vec::with_capacity(words.len());
        let mut skip = 0;
        for (index, word) in words.iter().enumerate() {
            if skip > 0 {
                skip -= 1;
                continue;
            }
            let upper = word.to_ascii_uppercase();
            let next = words.get(index + 1).map(|w| w.to_ascii_uppercase());
            if upper == "CHARSET" {
                skip = 1;
            } else if upper == "CHARACTER" && next.as_deref() == Some("SET") {
                skip = 2;
            } else {
                kept.push(word);
            }
        }
        self.data_type = kept.join(" ");
        if let Some(charset) = charset {
            self.data_type = format!("{} CHARACTER SET {charset}", self.data_type);
        }
        self.remove(&[Clause::Collate]);
        if let Some(collation) = collation {
            self.clauses
                .insert(0, (Clause::Collate, format!("COLLATE {collation}")));
        }
    }

    /// Add `AUTO_INCREMENT` after the nullability and default clauses, or
    /// remove it.
    pub fn set_auto_increment(&mut self, on: bool) {
        self.clauses.retain(|(clause, text)| {
            !(*clause == Clause::Other && text.eq_ignore_ascii_case("AUTO_INCREMENT"))
        });
        if on {
            let at = self.after(&[
                Clause::Collate,
                Clause::Generated,
                Clause::NotNull,
                Clause::Null,
                Clause::Default,
            ]);
            self.clauses
                .insert(at, (Clause::Other, "AUTO_INCREMENT".to_owned()));
        }
    }

    /// Remove a column-level `REFERENCES …` foreign key with its actions
    /// (`ON DELETE …`, `MATCH …`, `DEFERRABLE …`); true when one was.
    pub fn remove_references(&mut self) -> bool {
        let before = self.clauses.len();
        let mut removing = false;
        self.clauses.retain(|(clause, text)| {
            let upper = text.to_ascii_uppercase();
            let starts = *clause == Clause::Other
                && (upper.starts_with("REFERENCES ")
                    || (upper.starts_with("CONSTRAINT ") && upper.contains(" REFERENCES ")));
            let action = *clause == Clause::Other
                && [
                    "ON DELETE",
                    "ON UPDATE",
                    "MATCH ",
                    "DEFERRABLE",
                    "INITIALLY ",
                ]
                .iter()
                .any(|prefix| upper.starts_with(prefix));
            removing = starts || (removing && action);
            !removing
        });
        self.clauses.len() < before
    }

    /// The definition as one line.
    #[must_use]
    pub fn render(&self) -> String {
        let mut text = format!("{} {}", self.name, self.data_type);
        for (_, clause) in &self.clauses {
            text.push(' ');
            text.push_str(clause);
        }
        text
    }

    /// Remove every clause of `kinds`; the index of the first one removed.
    fn remove(&mut self, kinds: &[Clause]) -> Option<usize> {
        let first = self
            .clauses
            .iter()
            .position(|(clause, _)| kinds.contains(clause));
        self.clauses.retain(|(clause, _)| !kinds.contains(clause));
        first
    }

    /// Index just past the last leading clause of `kinds`.
    fn after(&self, kinds: &[Clause]) -> usize {
        self.clauses
            .iter()
            .take_while(|(clause, _)| kinds.contains(clause))
            .count()
    }
}

/// Clauses collected token by token, white space collapsed to one space.
#[derive(Default)]
struct Parts {
    data_type: String,
    clauses: Vec<(Clause, String)>,
    depth: usize,
    /// The previous significant word, upper-cased.
    previous: String,
}

impl Parts {
    fn push(&mut self, text: &str, token: Token) {
        let word = token.text(text);
        if token.kind == TokenKind::Whitespace {
            self.append(" ");
            return;
        }
        if self.depth == 0 && token.kind == TokenKind::Word {
            self.clause_boundary(word);
        }
        match token.kind {
            TokenKind::OpenParen => self.depth += 1,
            TokenKind::CloseParen => self.depth = self.depth.saturating_sub(1),
            _ => {}
        }
        self.append(word);
        if token.kind == TokenKind::Word {
            self.previous = word.to_ascii_uppercase();
        }
    }

    /// Start a clause at `word` when it opens one; `CONSTRAINT name` takes
    /// the kind of the clause it names, and `ON CONFLICT` stays with the
    /// constraint before it (`SQLite`).
    fn clause_boundary(&mut self, word: &str) {
        if word.eq_ignore_ascii_case("CONFLICT") && self.previous == "ON" {
            self.rejoin_on();
            return;
        }
        if !self.starts_clause(word) {
            return;
        }
        let kind = clause_kind(word);
        match self.clauses.last_mut() {
            Some(last) if last.0 == Clause::Other && is_named_constraint(&last.1) => {
                last.0 = kind;
            }
            _ => self.clauses.push((kind, String::new())),
        }
    }

    /// Move a just-opened `ON` clause back into the clause before it.
    fn rejoin_on(&mut self) {
        let opened_on = self
            .clauses
            .last()
            .is_some_and(|(_, clause)| clause.trim().eq_ignore_ascii_case("ON"));
        if !opened_on {
            return;
        }
        if let Some((_, on)) = self.clauses.pop() {
            self.append(" ");
            self.append(on.trim());
            self.append(" ");
        }
    }

    fn append(&mut self, text: &str) {
        let target = match self.clauses.last_mut() {
            Some((_, clause)) => clause,
            None => &mut self.data_type,
        };
        if text == " " && (target.is_empty() || target.ends_with(' ')) {
            return;
        }
        target.push_str(text);
    }

    fn starts_clause(&self, word: &str) -> bool {
        let upper = word.to_ascii_uppercase();
        let continues = match upper.as_str() {
            "NULL" => matches!(self.previous.as_str(), "NOT" | "DEFAULT" | "IS"),
            "AS" => self.previous == "ALWAYS",
            _ => false,
        };
        !continues && CLAUSE_WORDS.contains(&upper.as_str())
    }

    fn finish(self) -> Vec<(Clause, String)> {
        self.clauses
            .into_iter()
            .map(|(kind, text)| (kind, text.trim().to_owned()))
            .collect()
    }
}

fn is_named_constraint(open: &str) -> bool {
    let mut words = open.split_whitespace();
    words
        .next()
        .is_some_and(|word| word.eq_ignore_ascii_case("CONSTRAINT"))
        && words.count() <= 1
}

fn clause_kind(word: &str) -> Clause {
    match word.to_ascii_uppercase().as_str() {
        "NOT" => Clause::NotNull,
        "NULL" => Clause::Null,
        "DEFAULT" => Clause::Default,
        "COLLATE" => Clause::Collate,
        "COMMENT" => Clause::Comment,
        "GENERATED" | "AS" => Clause::Generated,
        _ => Clause::Other,
    }
}
