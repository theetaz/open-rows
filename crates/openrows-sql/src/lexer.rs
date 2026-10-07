// SPDX-License-Identifier: Apache-2.0
//! Hand-written `PostgreSQL` lexer shared by the splitter, classifier and
//! placeholder scanner.
//!
//! The lexer never fails: every byte of the input belongs to exactly one token,
//! and unterminated strings, quoted identifiers, dollar quotes and block
//! comments extend to the end of the input. Token boundaries always fall on
//! ASCII bytes or the end of input, so they are valid `str` slice boundaries.

/// Lexical category of a [`Token`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TokenKind {
    /// ASCII whitespace run.
    Whitespace,
    /// `-- …` up to (not including) the line break.
    LineComment,
    /// `/* … */`, nesting aware.
    BlockComment,
    /// Unquoted identifier or keyword.
    Word,
    /// `"…"` with `""` escapes.
    QuotedIdent,
    /// `'…'` or `E'…'` string literal.
    String,
    /// `$$…$$` or `$tag$…$tag$` string.
    DollarString,
    /// Positional parameter such as `$1`.
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

/// Iterator over the tokens of a `PostgreSQL` source text.
pub(crate) struct Lexer<'a> {
    source: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub(crate) fn new(source: &'a str) -> Self {
        Self { source, pos: 0 }
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

    fn block_comment_end(&self) -> usize {
        let mut depth = 0_usize;
        let mut index = self.pos;
        while let Some(byte) = self.byte(index) {
            let next = self.byte(index + 1);
            if byte == b'/' && next == Some(b'*') {
                depth += 1;
                index += 2;
            } else if byte == b'*' && next == Some(b'/') && depth == 1 {
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

    fn word(&self) -> Token {
        let end = self.scan_while(self.pos, is_ident_continue);
        let is_escape_prefix = end == self.pos + 1
            && matches!(self.byte(self.pos), Some(b'e' | b'E'))
            && self.byte(end) == Some(b'\'');
        if is_escape_prefix {
            let string_end = self.quoted_end(end, b'\'', true);
            return self.token(TokenKind::String, string_end);
        }
        self.token(TokenKind::Word, end)
    }

    fn dollar(&self) -> Token {
        let after = self.pos + 1;
        if self.byte(after).is_some_and(|byte| byte.is_ascii_digit()) {
            let end = self.scan_while(after, |byte| byte.is_ascii_digit());
            return self.token(TokenKind::Param, end);
        }
        let tag_end = self.scan_while(after, |byte| is_ident_continue(byte) && byte != b'$');
        let valid_tag = tag_end == after || self.byte(after).is_some_and(is_ident_start);
        if !valid_tag || self.byte(tag_end) != Some(b'$') {
            return self.token(TokenKind::Punct, after);
        }
        let body = tag_end + 1;
        let delimiter = self.source.get(self.pos..body).unwrap_or_default();
        let end = self
            .source
            .get(body..)
            .and_then(|rest| rest.find(delimiter))
            .map_or(self.source.len(), |offset| body + offset + delimiter.len());
        self.token(TokenKind::DollarString, end)
    }

    fn token(&self, kind: TokenKind, end: usize) -> Token {
        let end = end.clamp(self.pos + 1, self.source.len());
        Token {
            kind,
            start: self.pos,
            end,
        }
    }

    fn next_token(&self, byte: u8) -> Token {
        let next = self.byte(self.pos + 1);
        match byte {
            b if b.is_ascii_whitespace() => {
                let end = self.scan_while(self.pos, |b| b.is_ascii_whitespace());
                self.token(TokenKind::Whitespace, end)
            }
            b'-' if next == Some(b'-') => {
                self.token(TokenKind::LineComment, self.line_comment_end())
            }
            b'/' if next == Some(b'*') => {
                self.token(TokenKind::BlockComment, self.block_comment_end())
            }
            b'\'' => self.token(TokenKind::String, self.quoted_end(self.pos, b'\'', false)),
            b'"' => self.token(
                TokenKind::QuotedIdent,
                self.quoted_end(self.pos, b'"', false),
            ),
            b'$' => self.dollar(),
            b if is_ident_start(b) => self.word(),
            b if b.is_ascii_digit() => {
                let end = self.scan_while(self.pos, |b| {
                    b.is_ascii_alphanumeric() || b == b'_' || b == b'.'
                });
                self.token(TokenKind::Number, end)
            }
            _ => self.token(single_byte_kind(byte), self.pos + 1),
        }
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

fn single_byte_kind(byte: u8) -> TokenKind {
    match byte {
        b';' => TokenKind::Semicolon,
        b'(' => TokenKind::OpenParen,
        b')' => TokenKind::CloseParen,
        b'[' => TokenKind::OpenBracket,
        b']' => TokenKind::CloseBracket,
        _ => TokenKind::Punct,
    }
}

/// `PostgreSQL` identifier start: letter, underscore or any non-ASCII byte.
fn is_ident_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_' || !byte.is_ascii()
}

fn is_ident_continue(byte: u8) -> bool {
    is_ident_start(byte) || byte.is_ascii_digit() || byte == b'$'
}

/// Tokens of `source` that are not whitespace or comments.
pub(crate) fn significant(source: &str) -> Vec<Token> {
    Lexer::new(source)
        .filter(|token| !token.kind.is_trivia())
        .collect()
}
