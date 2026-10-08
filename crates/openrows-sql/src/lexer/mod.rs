// SPDX-License-Identifier: Apache-2.0
//! Hand-written, dialect-aware SQL lexer shared by the splitter, classifier
//! and placeholder scanner.
//!
//! The lexer never fails: every byte of the input belongs to exactly one token,
//! and unterminated strings, quoted identifiers, dollar quotes and block
//! comments extend to the end of the input. Token boundaries always fall on
//! ASCII bytes or the end of input, so they are valid `str` slice boundaries.
//! The per-dialect byte rules live in [`rules`].

mod rules;

use crate::Dialect;

/// Lexical category of a [`Token`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TokenKind {
    /// ASCII whitespace run.
    Whitespace,
    /// `-- …` (and `MySQL` `# …`) up to (not including) the line break.
    LineComment,
    /// `/* … */` (nesting aware for `PostgreSQL`), and the `/*!` opener and
    /// `*/` closer of a `MySQL` executable comment, whose body is lexed as code.
    BlockComment,
    /// Unquoted identifier or keyword.
    Word,
    /// `"…"`, `` `…` `` or `[…]` quoted identifier.
    QuotedIdent,
    /// `'…'` (and `MySQL` `"…"`) string literal.
    String,
    /// `PostgreSQL` `$$…$$` or `$tag$…$tag$` string.
    DollarString,
    /// Positional or named parameter such as `$1`, `?2` or `$name`.
    Param,
    /// Numeric literal.
    Number,
    /// Statement terminator `;`.
    Semicolon,
    /// `(`.
    OpenParen,
    /// `)`.
    CloseParen,
    /// `[`.
    OpenBracket,
    /// `]`.
    CloseBracket,
    /// Any other single byte (operators, commas, colons, …).
    Punct,
}

impl TokenKind {
    /// Whitespace and comments carry no meaning for classification.
    pub(crate) fn is_trivia(self) -> bool {
        matches!(
            self,
            Self::Whitespace | Self::LineComment | Self::BlockComment
        )
    }
}

/// A lexical token: kind plus its byte range in the source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Token {
    pub(crate) kind: TokenKind,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

impl Token {
    /// Source text of this token.
    pub(crate) fn text(self, source: &str) -> &str {
        source.get(self.start..self.end).unwrap_or_default()
    }

    /// Whether this token is the unquoted word `keyword` (ASCII case-insensitive).
    pub(crate) fn is_word(self, source: &str, keyword: &str) -> bool {
        self.kind == TokenKind::Word && self.text(source).eq_ignore_ascii_case(keyword)
    }
}

/// Iterator over the tokens of a source text in one dialect.
pub(crate) struct Lexer<'a> {
    source: &'a str,
    pos: usize,
    dialect: Dialect,
    /// Inside a `MySQL` `/*! … */` executable comment.
    executable: bool,
}

impl<'a> Lexer<'a> {
    pub(crate) fn new(source: &'a str, dialect: Dialect) -> Self {
        Self {
            source,
            pos: 0,
            dialect,
            executable: false,
        }
    }

    /// Continue lexing `source` from byte `pos` (a token boundary).
    pub(crate) fn resume(source: &'a str, pos: usize, dialect: Dialect) -> Self {
        Self {
            pos,
            ..Self::new(source, dialect)
        }
    }

    fn byte(&self, index: usize) -> Option<u8> {
        self.source.as_bytes().get(index).copied()
    }

    fn scan_while(&self, from: usize, keep: impl Fn(u8) -> bool) -> usize {
        let bytes = self.source.as_bytes();
        let mut index = from;
        while bytes.get(index).is_some_and(|&byte| keep(byte)) {
            index += 1;
        }
        index
    }

    fn line_comment_end(&self) -> usize {
        self.scan_while(self.pos, |byte| byte != b'\n')
    }

    /// End of a block comment; `PostgreSQL` comments nest, others do not.
    fn block_comment_end(&self, nested: bool) -> usize {
        let mut depth = 0_usize;
        let mut index = self.pos;
        while let Some(byte) = self.byte(index) {
            let next = self.byte(index + 1);
            if byte == b'/' && next == Some(b'*') && (nested || depth == 0) {
                depth += 1;
                index += 2;
            } else if byte == b'*' && next == Some(b'/') && depth <= 1 {
                return index + 2;
            } else if byte == b'*' && next == Some(b'/') {
                depth -= 1;
                index += 2;
            } else {
                index += 1;
            }
        }
        self.source.len()
    }

    /// End of a quoted run opened at `open` (the quote byte itself).
    fn quoted_end(&self, open: usize, quote: u8, backslash_escapes: bool) -> usize {
        let mut index = open + 1;
        while let Some(byte) = self.byte(index) {
            let escaped = backslash_escapes && byte == b'\\';
            let doubled = byte == quote && self.byte(index + 1) == Some(quote);
            if escaped || doubled {
                index += 2;
            } else if byte == quote {
                return index + 1;
            } else {
                index += 1;
            }
        }
        self.source.len()
    }

    /// End of a run closed by `close` with no escapes (`SQLite` `[ident]`).
    fn until_end(&self, close: u8) -> usize {
        let end = self.scan_while(self.pos + 1, |byte| byte != close);
        (end + 1).min(self.source.len())
    }

    fn token(&self, kind: TokenKind, end: usize) -> Token {
        let end = end.clamp(self.pos + 1, self.source.len());
        Token {
            kind,
            start: self.pos,
            end,
        }
    }

    fn whitespace(&self) -> Token {
        let end = self.scan_while(self.pos, |b| b.is_ascii_whitespace());
        self.token(TokenKind::Whitespace, end)
    }

    fn number(&self) -> Token {
        let end = self.scan_while(self.pos, |b| {
            b.is_ascii_alphanumeric() || b == b'_' || b == b'.'
        });
        self.token(TokenKind::Number, end)
    }

    fn plain_word(&self) -> Token {
        let end = self.scan_while(self.pos, is_ident_continue);
        self.token(TokenKind::Word, end)
    }
}

impl Iterator for Lexer<'_> {
    type Item = Token;

    fn next(&mut self) -> Option<Token> {
        let byte = self.byte(self.pos)?;
        let token = self.next_token(byte);
        self.pos = token.end;
        Some(token)
    }
}

pub(crate) fn single_byte_kind(byte: u8) -> TokenKind {
    match byte {
        b';' => TokenKind::Semicolon,
        b'(' => TokenKind::OpenParen,
        b')' => TokenKind::CloseParen,
        b'[' => TokenKind::OpenBracket,
        b']' => TokenKind::CloseBracket,
        _ => TokenKind::Punct,
    }
}

/// Identifier start: letter, underscore or any non-ASCII byte.
pub(crate) fn is_ident_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_' || !byte.is_ascii()
}

pub(crate) fn is_ident_continue(byte: u8) -> bool {
    is_ident_start(byte) || byte.is_ascii_digit() || byte == b'$'
}

/// Tokens of `source` that are not whitespace or comments.
pub(crate) fn significant(source: &str, dialect: Dialect) -> Vec<Token> {
    Lexer::new(source, dialect)
        .filter(|token| !token.kind.is_trivia())
        .collect()
}
