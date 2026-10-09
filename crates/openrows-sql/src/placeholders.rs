// SPDX-License-Identifier: Apache-2.0
//! Parameter placeholder detection (positions only; no binding).

use std::ops::Range;

use crate::Dialect;
use crate::lexer::{Lexer, Token, TokenKind};

/// Placeholder syntax.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum PlaceholderKind {
    /// `$1`, `$2`, … (`PostgreSQL` native) or `SQLite` `?1`.
    Positional,
    /// `:name`.
    Named,
    /// `?`. In `PostgreSQL` `?` is also a `jsonb` operator, so callers may ignore
    /// this kind for that dialect.
    Anonymous,
}

/// One placeholder occurrence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Placeholder {
    /// Placeholder syntax.
    pub kind: PlaceholderKind,
    /// Byte range in the scanned text, including the sigil (`$`, `:`, `?`).
    pub range: Range<usize>,
}

/// Find `$n`, `:name` and `?` placeholders (and `SQLite` `?NNN` and
/// `$name`) outside strings, quoted identifiers, dollar quotes and comments,
/// in source order.
///
/// Not placeholders: `::type` casts, `:=`, array slices `a[lo:hi]`, psql
/// variables `:'v'`/`:"v"`, and the `jsonb` operators `?|`, `?&`.
#[must_use]
pub fn placeholders(source: &str, dialect: Dialect) -> Vec<Placeholder> {
    let tokens: Vec<Token> = Lexer::new(source, dialect).collect();
    Scanner {
        source,
        tokens: &tokens,
    }
    .scan()
}

struct Scanner<'a> {
    source: &'a str,
    tokens: &'a [Token],
}

impl Scanner<'_> {
    fn scan(&self) -> Vec<Placeholder> {
        let mut bracket_depth = 0_usize;
        let mut found = Vec::new();
        for (index, token) in self.tokens.iter().enumerate() {
            match token.kind {
                TokenKind::OpenBracket => bracket_depth += 1,
                TokenKind::CloseBracket => bracket_depth = bracket_depth.saturating_sub(1),
                TokenKind::Param => {
                    found.push(placeholder(param_kind(token.text(self.source)), *token));
                }
                TokenKind::Punct => found.extend(self.punct(index, bracket_depth)),
                _ => {}
            }
        }
        found
    }

    fn punct(&self, index: usize, bracket_depth: usize) -> Option<Placeholder> {
        let token = *self.tokens.get(index)?;
        match token.text(self.source) {
            ":" if bracket_depth == 0 => self.named(index, token),
            "?" if !self.glued(index, |text| matches!(text, "|" | "&" | "?")) => {
                Some(placeholder(PlaceholderKind::Anonymous, token))
            }
            _ => None,
        }
    }

    fn named(&self, index: usize, colon: Token) -> Option<Placeholder> {
        let name = *self.tokens.get(index + 1)?;
        let follows_colon = index
            .checked_sub(1)
            .and_then(|previous| self.tokens.get(previous))
            .is_some_and(|previous| {
                previous.end == colon.start && previous.text(self.source) == ":"
            });
        let named = name.kind == TokenKind::Word && name.start == colon.end && !follows_colon;
        named.then_some(Placeholder {
            kind: PlaceholderKind::Named,
            range: colon.start..name.end,
        })
    }

    /// Whether the token at `index` touches a neighbour whose text matches.
    fn glued(&self, index: usize, matches: impl Fn(&str) -> bool) -> bool {
        let neighbours = [index.checked_sub(1), index.checked_add(1)];
        neighbours
            .into_iter()
            .flatten()
            .filter_map(|neighbour| self.tokens.get(neighbour))
            .any(|neighbour| matches(neighbour.text(self.source)))
    }
}

/// `$1` and `SQLite` `?1` are positional; `SQLite` `$name` is named.
fn param_kind(text: &str) -> PlaceholderKind {
    if text.starts_with('{') {
        return PlaceholderKind::Named;
    }
    let numbered = text
        .get(1..)
        .is_some_and(|rest| rest.bytes().all(|byte| byte.is_ascii_digit()));
    if numbered {
        PlaceholderKind::Positional
    } else {
        PlaceholderKind::Named
    }
}

fn placeholder(kind: PlaceholderKind, token: Token) -> Placeholder {
    Placeholder {
        kind,
        range: token.start..token.end,
    }
}
