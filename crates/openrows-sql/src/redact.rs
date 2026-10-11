// SPDX-License-Identifier: Apache-2.0
//! History literal removal using the same lexer as execution safety.
use crate::{
    Dialect,
    lexer::{Lexer, TokenKind},
};
use std::fmt::Write as _;
use std::ops::Range;
/// Marker deliberately blocked by query execution until edited away.
pub const REDACTED_PREFIX: &str = "__OPENROWS_REDACTED_";
/// Byte ranges of string, dollar-quoted and numeric literals, never identifiers.
#[must_use]
pub fn literal_ranges(sql: &str, dialect: Dialect) -> Vec<Range<usize>> {
    Lexer::new(sql, dialect)
        .filter(|token| {
            matches!(
                token.kind,
                TokenKind::String | TokenKind::DollarString | TokenKind::Number
            )
        })
        .map(|token| token.start..token.end)
        .collect()
}
/// Replace literals with visibly unfilled markers and omit comment payloads.
#[must_use]
pub fn redact_literals(sql: &str, dialect: Dialect) -> String {
    let mut output = String::with_capacity(sql.len());
    let mut index = 0;
    for token in Lexer::new(sql, dialect) {
        match token.kind {
            TokenKind::String | TokenKind::DollarString | TokenKind::Number => {
                index += 1;
                let _ = write!(output, "{REDACTED_PREFIX}{index}__");
            }
            TokenKind::LineComment | TokenKind::BlockComment => {
                output.push(' ');
            }
            _ => output.push_str(token.text(sql)),
        }
    }
    output
}
/// Conservatively block remaining markers even in comments or strings.
#[must_use]
pub fn has_redacted_literals(sql: &str) -> bool {
    sql.as_bytes()
        .windows(REDACTED_PREFIX.len())
        .any(|window| window.eq_ignore_ascii_case(REDACTED_PREFIX.as_bytes()))
}
/// Replace matching identifiers; hide boolean/NULL literals and ambiguous `SQLite` double quotes.
#[must_use]
pub fn redact_identifiers(sql: &str, dialect: Dialect, sensitive: impl Fn(&str) -> bool) -> String {
    let tokens: Vec<_> = Lexer::new(sql, dialect).collect();
    let mut output = String::with_capacity(sql.len());
    let mut index = 0;
    while let Some(token) = tokens.get(index) {
        if !matches!(token.kind, TokenKind::Word | TokenKind::QuotedIdent) {
            output.push_str(token.text(sql));
            index += 1;
            continue;
        }
        let mut end = index + 1;
        let mut name = identifier_name(token.text(sql)).to_owned();
        while let Some(next) = qualified_next(&tokens, end, sql) {
            name.push('.');
            name.push_str(identifier_name(tokens[next].text(sql)));
            end = next + 1;
        }
        let ambiguous = tokens[index..end]
            .iter()
            .any(|part| ambiguous_identifier(part.text(sql), dialect));
        if sensitive(&name) || name.split('.').any(&sensitive) || ambiguous {
            output.push_str("‹redacted›");
        } else {
            output.push_str(&sql[token.start..tokens[end - 1].end]);
        }
        index = end;
    }
    output
}
fn ambiguous_identifier(text: &str, dialect: Dialect) -> bool {
    if ["true", "false", "null"]
        .iter()
        .any(|literal| text.eq_ignore_ascii_case(literal))
    {
        return true;
    }
    if dialect == Dialect::Sqlite && text.starts_with('"') {
        return true;
    }
    match text.as_bytes().first() {
        Some(b'"') => !text.ends_with('"') || text.len() < 2,
        Some(b'`') => !text.ends_with('`') || text.len() < 2,
        Some(b'[') => !text.ends_with(']') || text.len() < 2,
        _ => false,
    }
}
fn identifier_name(text: &str) -> &str {
    text.trim_matches(['"', '`', '[', ']'])
}
fn qualified_next(tokens: &[crate::lexer::Token], from: usize, sql: &str) -> Option<usize> {
    let dot = (from..tokens.len()).find(|&index| tokens[index].kind != TokenKind::Whitespace)?;
    if tokens[dot].text(sql) != "." {
        return None;
    }
    let next =
        (dot + 1..tokens.len()).find(|&index| tokens[index].kind != TokenKind::Whitespace)?;
    matches!(tokens[next].kind, TokenKind::Word | TokenKind::QuotedIdent).then_some(next)
}

/// Fail-closed monitoring normalisation: utility/truncated syntax has no text.
#[must_use]
pub fn normalize_monitor_query(sql: &str) -> Option<String> {
    if sql.len() >= 4_096 || sql.contains('\0') {
        return None;
    }
    let tokens: Vec<_> = Lexer::new(sql, Dialect::Postgres).collect();
    let first = tokens.iter().find(|token| !token.kind.is_trivia())?;
    if !["SELECT", "WITH", "INSERT", "UPDATE", "DELETE"]
        .iter()
        .any(|word| first.is_word(sql, word))
    {
        return None;
    }
    let mut output = String::new();
    let mut ended = false;
    for token in tokens {
        let text = token.text(sql);
        if (ended && !token.kind.is_trivia()) || !monitor_token_complete(text, token.kind) {
            return None;
        }
        ended |= token.kind == TokenKind::Semicolon;
        match token.kind {
            TokenKind::String | TokenKind::DollarString | TokenKind::Number | TokenKind::Param => {
                output.push('?');
            }
            TokenKind::LineComment | TokenKind::BlockComment => output.push(' '),
            TokenKind::Word
                if ["true", "false", "null"]
                    .iter()
                    .any(|word| text.eq_ignore_ascii_case(word)) =>
            {
                output.push('?');
            }
            TokenKind::Word
            | TokenKind::Whitespace
            | TokenKind::Semicolon
            | TokenKind::OpenParen
            | TokenKind::CloseParen
            | TokenKind::OpenBracket
            | TokenKind::CloseBracket => output.push_str(text),
            TokenKind::Punct if text.bytes().all(|byte| b",.:+-*/%=<>!|&^~".contains(&byte)) => {
                output.push_str(text);
            }
            TokenKind::QuotedIdent | TokenKind::Punct => return None,
        }
    }
    Some(output.split_whitespace().collect::<Vec<_>>().join(" "))
}
fn monitor_token_complete(text: &str, kind: TokenKind) -> bool {
    match kind {
        TokenKind::String => text.ends_with('\'') && text.len() > 1 && !text.ends_with("\\\'"),
        TokenKind::DollarString => {
            text.get(1..)
                .and_then(|body| body.find('$'))
                .is_some_and(|end| {
                    let delimiter = &text[..end + 2];
                    text.len() >= delimiter.len() * 2 && text.ends_with(delimiter)
                })
        }
        TokenKind::BlockComment => text.ends_with("*/"),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn removes_strings_numbers_dollar_quotes_and_comment_payloads() {
        let sql = "SELECT 'x', E'x', 12.3e-4, $$x$$, $tag$x$tag$, \"ident\", $1 -- x";
        let redacted = redact_literals(sql, Dialect::Postgres);
        assert!(!redacted.contains("'x'"));
        assert!(!redacted.contains("12.3"));
        assert!(!redacted.contains("$tag$"));
        assert!(!redacted.contains("-- x"));
        assert!(redacted.contains("\"ident\", $1"));
        assert_eq!(redacted.matches(REDACTED_PREFIX).count(), 5);
        assert!(has_redacted_literals(&redacted));
    }
    #[test]
    fn dialect_quotes_and_unicode_ranges() {
        for (sql, dialect) in [
            ("SELECT N'x', 0x12, [column]", Dialect::SqlServer),
            ("SELECT \"x\", 'x', `column`", Dialect::MySql),
            ("SELECT 'සිංහල', .25, [column]", Dialect::Sqlite),
        ] {
            let redacted = redact_literals(sql, dialect);
            assert!(!redacted.contains("'x'"));
            assert!(!redacted.contains("සිංහල"));
            assert_eq!(literal_ranges(sql, dialect).len(), 2);
        }
    }
}
