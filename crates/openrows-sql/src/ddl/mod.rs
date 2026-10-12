// SPDX-License-Identifier: Apache-2.0
//! Building blocks for structure-change DDL (docs/10 §3): type-name and
//! literal checks per dialect, a `CREATE TABLE` statement split into its
//! column and constraint definitions ([`TableDefinition`]) with editable
//! column definitions ([`ColumnDefinition`]), and schema-qualifying the
//! name of a stored `CREATE INDEX` / `CREATE TRIGGER` statement.
mod column;
mod created;
mod table;

pub use column::ColumnDefinition;
pub use created::{created_kind, view_body};
pub use table::{TableDefinition, TableElement};

use crate::Dialect;
use crate::lexer::{Lexer, TokenKind};

/// Whether `PostgreSQL` text is exactly one conversion expression.
#[must_use]
pub fn valid_using_expression(sql: &str) -> bool {
    use sqlparser::{dialect::PostgreSqlDialect, parser::Parser, tokenizer::Token};
    if sql.trim().is_empty() || sql.len() > 65_536 || sql.contains('\0') {
        return false;
    }
    if Lexer::new(sql, Dialect::Postgres)
        .any(|token| matches!(token.kind, TokenKind::LineComment | TokenKind::BlockComment))
    {
        return false;
    }
    let Ok(mut parser) = Parser::new(&PostgreSqlDialect {})
        .with_recursion_limit(64)
        .try_with_sql(sql)
    else {
        return false;
    };
    parser.parse_expr().is_ok() && parser.peek_token().token == Token::EOF
}

/// Longest accepted type name.
const MAX_TYPE: usize = 200;

/// `text` as a string literal, or `None` when it holds a NUL character.
/// `MySQL` escapes backslashes too (its default `sql_mode` reads them);
/// SQL Server literals are Unicode (`N'…'`).
#[must_use]
pub fn string_literal(dialect: Dialect, text: &str) -> Option<String> {
    if text.contains('\0') {
        return None;
    }
    let doubled = text.replace('\'', "''");
    Some(match dialect {
        Dialect::MySql | Dialect::ClickHouse => format!("'{}'", doubled.replace('\\', "\\\\")),
        Dialect::Postgres | Dialect::Duckdb | Dialect::Sqlite => format!("'{doubled}'"),
        Dialect::SqlServer => format!("N'{doubled}'"),
    })
}

/// Type syntax: words (`double precision`, `int unsigned`), quoted or
/// schema-qualified names, integer modifiers (`numeric(10, 2)`) and array
/// suffixes (`text[]`); for `MySQL` also string lists (`enum('a', 'b')`),
/// for SQL Server `max` lengths (`nvarchar(max)`).
/// Nothing else: no operators, comments or `;`.
#[must_use]
pub fn valid_type(dialect: Dialect, text: &str) -> bool {
    if text.is_empty() || text.len() > MAX_TYPE || text.contains('\0') {
        return false;
    }
    let mut state = TypeScan::default();
    for token in Lexer::new(text, dialect) {
        if !state.accept(dialect, token.kind, token.text(text)) {
            return false;
        }
    }
    state.depth == 0 && !state.bracket && state.words > 0
}

/// Nesting and word count while scanning a type name.
#[derive(Default)]
struct TypeScan {
    /// Inside `( … )` (one level only).
    depth: u32,
    /// Inside `[ … ]`.
    bracket: bool,
    words: u32,
}

impl TypeScan {
    fn accept(&mut self, dialect: Dialect, kind: TokenKind, text: &str) -> bool {
        let outside = self.depth == 0 && !self.bracket;
        match kind {
            TokenKind::Whitespace => true,
            TokenKind::Word if outside => {
                self.words += 1;
                true
            }
            TokenKind::Word => {
                self.depth > 0 && dialect == Dialect::SqlServer && text.eq_ignore_ascii_case("max")
            }
            TokenKind::QuotedIdent if outside => {
                self.words += 1;
                quoted_ident(dialect, text)
            }
            TokenKind::Number => !outside && text.bytes().all(|b| b.is_ascii_digit()),
            TokenKind::String => self.depth > 0 && dialect == Dialect::MySql,
            TokenKind::OpenParen if outside => {
                self.depth = 1;
                true
            }
            TokenKind::CloseParen if self.depth == 1 => {
                self.depth = 0;
                true
            }
            TokenKind::OpenBracket if outside && dialect == Dialect::Postgres => {
                self.bracket = true;
                true
            }
            TokenKind::CloseBracket if self.bracket => {
                self.bracket = false;
                true
            }
            TokenKind::Punct => match text {
                "." => outside,
                "," | "-" => self.depth > 0,
                _ => false,
            },
            _ => false,
        }
    }
}

/// A terminated quoted identifier of `dialect`.
fn quoted_ident(dialect: Dialect, text: &str) -> bool {
    let closed = |open: char, close: char| {
        text.len() >= 2 && text.starts_with(open) && text.ends_with(close)
    };
    let brackets = matches!(dialect, Dialect::Sqlite | Dialect::SqlServer);
    closed('"', '"') || closed('`', '`') || (brackets && closed('[', ']'))
}

/// A stored `CREATE [UNIQUE] INDEX` or `CREATE TRIGGER` statement with its
/// object name qualified by `schema` (already quoted), so it is created in
/// that attached database again. Already qualified names are kept.
#[must_use]
pub fn qualify_created_name(sql: &str, dialect: Dialect, schema: &str) -> Option<String> {
    let tokens = crate::lexer::significant(sql, dialect);
    let kind = tokens
        .iter()
        .position(|token| token.is_word(sql, "INDEX") || token.is_word(sql, "TRIGGER"))?;
    let mut at = kind + 1;
    let is = |index: usize, word: &str| tokens.get(index).is_some_and(|t| t.is_word(sql, word));
    if is(at, "IF") && is(at + 1, "NOT") && is(at + 2, "EXISTS") {
        at += 3;
    }
    let name = tokens.get(at)?;
    if tokens.get(at + 1).is_some_and(|next| next.text(sql) == ".") {
        return Some(sql.to_owned());
    }
    Some(format!(
        "{}{schema}.{}",
        &sql[..name.start],
        &sql[name.start..]
    ))
}

/// The unquoted name of an identifier token's text.
pub(crate) fn unquote(text: &str) -> String {
    let inner = |open: char, close: char| {
        text.strip_prefix(open)
            .and_then(|rest| rest.strip_suffix(close))
            .map(|inner| inner.replace(&format!("{close}{close}"), &close.to_string()))
    };
    inner('"', '"')
        .or_else(|| inner('`', '`'))
        .or_else(|| inner('[', ']'))
        .unwrap_or_else(|| text.to_owned())
}

#[cfg(test)]
mod tests;
