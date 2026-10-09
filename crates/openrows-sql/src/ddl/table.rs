// SPDX-License-Identifier: Apache-2.0
//! A `CREATE TABLE … ( … ) options` statement split at its top-level commas.
use super::unquote;
use crate::Dialect;
use crate::lexer::{Lexer, Token, TokenKind};
use std::ops::Range;

/// Words that open a table constraint (or a `MySQL` index) instead of a column.
const CONSTRAINT_WORDS: [&str; 10] = [
    "CONSTRAINT",
    "PRIMARY",
    "UNIQUE",
    "CHECK",
    "FOREIGN",
    "KEY",
    "INDEX",
    "FULLTEXT",
    "SPATIAL",
    "EXCLUDE",
];

/// One definition between the table's parentheses, as written.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TableElement {
    /// The text between its commas, including surrounding white space and comments.
    pub text: String,
    /// The column's unquoted name; `None` for a table constraint or index.
    pub column: Option<String>,
}

impl TableElement {
    /// The text split into leading trivia (white space and comments), the
    /// definition itself and trailing trivia.
    #[must_use]
    pub fn parts(&self, dialect: Dialect) -> (&str, &str, &str) {
        let significant: Vec<Token> = Lexer::new(&self.text, dialect)
            .filter(|token| !token.kind.is_trivia())
            .collect();
        let (Some(first), Some(last)) = (significant.first(), significant.last()) else {
            return (self.text.as_str(), "", "");
        };
        let text = self.text.as_str();
        (
            text.get(..first.start).unwrap_or_default(),
            text.get(first.start..last.end).unwrap_or_default(),
            text.get(last.end..).unwrap_or_default(),
        )
    }
}

/// A `CREATE TABLE` statement as head, definitions and table options.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TableDefinition {
    /// `CREATE TABLE name (`.
    pub head: String,
    /// Column and constraint definitions in order.
    pub elements: Vec<TableElement>,
    /// `)` and the table options after it, e.g. `) WITHOUT ROWID`.
    pub tail: String,
}

impl TableDefinition {
    /// Split the first statement of `sql`; `None` when it is not a
    /// `CREATE TABLE` with a parenthesised definition list.
    #[must_use]
    pub fn parse(sql: &str, dialect: Dialect) -> Option<Self> {
        let statement = crate::split(sql, dialect).first()?.text(sql);
        let tokens: Vec<Token> = Lexer::new(statement, dialect).collect();
        let first = tokens.iter().find(|token| !token.kind.is_trivia())?;
        if !first.is_word(statement, "CREATE") {
            return None;
        }
        let open = tokens
            .iter()
            .position(|token| token.kind == TokenKind::OpenParen)?;
        let before_open = &tokens[..open];
        if !before_open
            .iter()
            .any(|token| token.is_word(statement, "TABLE"))
            || before_open
                .iter()
                .any(|token| token.is_word(statement, "AS"))
        {
            return None;
        }
        let (ranges, close) = split_elements(statement, &tokens, open)?;
        let elements = ranges
            .into_iter()
            .map(|range| element(statement, dialect, range))
            .collect();
        Some(Self {
            head: statement.get(..tokens[open].end)?.to_owned(),
            elements,
            tail: statement.get(tokens[close].start..)?.trim_end().to_owned(),
        })
    }

    /// The element defining column `name`.
    #[must_use]
    pub fn column(&self, name: &str) -> Option<&TableElement> {
        self.elements
            .iter()
            .find(|element| element.column.as_deref() == Some(name))
    }

    /// Head, elements joined by commas, and tail.
    #[must_use]
    pub fn render(&self) -> String {
        let body: Vec<&str> = self.elements.iter().map(|e| e.text.as_str()).collect();
        format!("{}{}{}", self.head, body.join(","), self.tail)
    }
}

/// Byte ranges of the top-level definitions after the `(` at `open`, and
/// the index of the matching `)`.
fn split_elements(
    statement: &str,
    tokens: &[Token],
    open: usize,
) -> Option<(Vec<Range<usize>>, usize)> {
    let mut depth = 0_usize;
    let mut start = tokens[open].end;
    let mut ranges = Vec::new();
    for (index, token) in tokens.iter().enumerate().skip(open + 1) {
        match token.kind {
            TokenKind::OpenParen => depth += 1,
            TokenKind::CloseParen if depth == 0 => {
                ranges.push(start..token.start);
                return Some((ranges, index));
            }
            TokenKind::CloseParen => depth -= 1,
            TokenKind::Punct if depth == 0 && token.text(statement) == "," => {
                ranges.push(start..token.start);
                start = token.end;
            }
            _ => {}
        }
    }
    None
}

fn element(statement: &str, dialect: Dialect, range: Range<usize>) -> TableElement {
    let text = statement.get(range).unwrap_or_default();
    let first = Lexer::new(text, dialect).find(|token| !token.kind.is_trivia());
    let column = first.and_then(|token| {
        let word = token.text(text);
        let constraint = token.kind == TokenKind::Word
            && CONSTRAINT_WORDS
                .iter()
                .any(|keyword| word.eq_ignore_ascii_case(keyword));
        let name = matches!(token.kind, TokenKind::Word | TokenKind::QuotedIdent);
        (name && !constraint).then(|| unquote(word))
    });
    TableElement {
        text: text.to_owned(),
        column,
    }
}
