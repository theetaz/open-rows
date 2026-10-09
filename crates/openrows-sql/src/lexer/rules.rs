// SPDX-License-Identifier: Apache-2.0
//! Per-dialect byte rules: comments, quoting and parameters.
//!
//! - `PostgreSQL`: nested `/* */`, `E'…'` escape strings, `$tag$` dollar
//!   quotes, `$1` parameters, `"…"` identifiers.
//! - `MySQL`/`MariaDB`: `#` and `-- ` comments, backslash escapes in `'…'` and
//!   `"…"` strings, `` `…` `` identifiers, and `/*! … */` executable comments
//!   whose body is lexed as code (the server runs it).
//! - `SQLite`: `"…"`, `` `…` `` and `[…]` identifiers, `?NNN` and `$name`
//!   parameters, no backslash escapes.
//! - SQL Server (T-SQL): nested `/* */`, `[…]` identifiers with `]]`
//!   escapes, `N'…'` strings, and `@variable`, `@@function`, `#temp` and
//!   `$action` names lexed as words (they are not bind parameters).

use super::{Lexer, Token, TokenKind, is_ident_continue, is_ident_start, single_byte_kind};
use crate::Dialect;

impl Lexer<'_> {
    pub(super) fn next_token(&mut self, byte: u8) -> Token {
        let special = match self.dialect {
            Dialect::Postgres => self.postgres(byte),
            Dialect::MySql => self.mysql(byte),
            Dialect::Sqlite => self.sqlite(byte),
            Dialect::SqlServer => self.sqlserver(byte),
            Dialect::ClickHouse => self.clickhouse(byte),
        };
        special.unwrap_or_else(|| self.common(byte))
    }

    /// Rules every dialect shares.
    fn common(&self, byte: u8) -> Token {
        let next = self.byte(self.pos + 1);
        match byte {
            b if b.is_ascii_whitespace() => self.whitespace(),
            b'-' if next == Some(b'-') => {
                self.token(TokenKind::LineComment, self.line_comment_end())
            }
            b'/' if next == Some(b'*') => {
                let nested = matches!(self.dialect, Dialect::Postgres | Dialect::SqlServer);
                self.token(TokenKind::BlockComment, self.block_comment_end(nested))
            }
            b'\'' => {
                let escapes = matches!(self.dialect, Dialect::MySql | Dialect::ClickHouse);
                self.token(TokenKind::String, self.quoted_end(self.pos, b'\'', escapes))
            }
            b'"' => self.token(
                TokenKind::QuotedIdent,
                self.quoted_end(self.pos, b'"', false),
            ),
            b if is_ident_start(b) => self.plain_word(),
            b'.' if next.is_some_and(|b| b.is_ascii_digit()) => self.number(),
            b if b.is_ascii_digit() => self.number(),
            _ => self.token(single_byte_kind(byte), self.pos + 1),
        }
    }

    fn clickhouse(&self, byte: u8) -> Option<Token> {
        match byte {
            b'`' | b'"' => Some(self.token(
                TokenKind::QuotedIdent,
                self.quoted_end(self.pos, byte, true),
            )),
            b'#' => Some(self.token(TokenKind::LineComment, self.line_comment_end())),
            b'{' => Some(self.token(TokenKind::Param, self.until_end(b'}'))),
            _ => None,
        }
    }

    fn postgres(&self, byte: u8) -> Option<Token> {
        match byte {
            b'$' => Some(self.dollar()),
            b'e' | b'E' if self.byte(self.pos + 1) == Some(b'\'') => {
                let end = self.quoted_end(self.pos + 1, b'\'', true);
                Some(self.token(TokenKind::String, end))
            }
            _ => None,
        }
    }

    fn mysql(&mut self, byte: u8) -> Option<Token> {
        let next = self.byte(self.pos + 1);
        let token = match byte {
            b'#' => self.token(TokenKind::LineComment, self.line_comment_end()),
            // `--` starts a comment only before whitespace, a control byte or the end.
            b'-' if next == Some(b'-') => {
                let after = self.byte(self.pos + 2);
                if after.is_none_or(|b| b.is_ascii_whitespace() || b.is_ascii_control()) {
                    self.token(TokenKind::LineComment, self.line_comment_end())
                } else {
                    self.token(TokenKind::Punct, self.pos + 1)
                }
            }
            b'*' if self.executable && next == Some(b'/') => {
                self.executable = false;
                self.token(TokenKind::BlockComment, self.pos + 2)
            }
            b'/' if next == Some(b'*') => return self.mysql_comment(),
            b'"' => self.token(TokenKind::String, self.quoted_end(self.pos, b'"', true)),
            b'`' => self.token(
                TokenKind::QuotedIdent,
                self.quoted_end(self.pos, b'`', false),
            ),
            b'$' if self.byte(self.pos + 1).is_some_and(is_ident_continue) => self.plain_word(),
            _ => return None,
        };
        Some(token)
    }

    /// `/*!50700 … */` and `/*M!100100 … */`: the opener (with its version)
    /// is trivia and the body is code. Plain `/* … */` is one comment.
    fn mysql_comment(&mut self) -> Option<Token> {
        let marker = match (self.byte(self.pos + 2), self.byte(self.pos + 3)) {
            (Some(b'!'), _) => 3,
            (Some(b'M'), Some(b'!')) => 4,
            _ => return None,
        };
        if self.executable {
            return None;
        }
        self.executable = true;
        let end = self.scan_while(self.pos + marker, |b| b.is_ascii_digit());
        Some(self.token(TokenKind::BlockComment, end))
    }

    fn sqlite(&self, byte: u8) -> Option<Token> {
        let next = self.byte(self.pos + 1);
        let token = match byte {
            b'`' => self.token(
                TokenKind::QuotedIdent,
                self.quoted_end(self.pos, b'`', false),
            ),
            b'[' => self.token(TokenKind::QuotedIdent, self.until_end(b']')),
            b'?' if next.is_some_and(|b| b.is_ascii_digit()) => {
                let end = self.scan_while(self.pos + 1, |b| b.is_ascii_digit());
                self.token(TokenKind::Param, end)
            }
            b'$' if next.is_some_and(is_ident_start) => {
                let end = self.scan_while(self.pos + 1, is_ident_continue);
                self.token(TokenKind::Param, end)
            }
            _ => return None,
        };
        Some(token)
    }

    fn sqlserver(&self, byte: u8) -> Option<Token> {
        let next = self.byte(self.pos + 1);
        let token = match byte {
            b'[' => self.token(TokenKind::QuotedIdent, self.quoted_close(b']')),
            b'N' | b'n' if next == Some(b'\'') => {
                let end = self.quoted_end(self.pos + 1, b'\'', false);
                self.token(TokenKind::String, end)
            }
            b'@' | b'#' | b'$' => {
                let sigils = self.scan_while(self.pos, |b| b == byte);
                let end = self.scan_while(sigils, is_ident_continue);
                if end == sigils {
                    return None;
                }
                self.token(TokenKind::Word, end)
            }
            _ => return None,
        };
        Some(token)
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
}
