// SPDX-License-Identifier: Apache-2.0
//! The `MySQL` client's `DELIMITER` command: a custom statement terminator,
//! recognised anywhere outside strings, quoted identifiers and comments.

use std::ops::Range;

use crate::lexer::{Token, TokenKind};

/// The active terminator; `None` is the default `;`.
#[derive(Debug, Default)]
pub(super) struct Delimiter(Option<String>);

impl Delimiter {
    /// True while `;` terminates statements.
    pub(super) fn is_default(&self) -> bool {
        self.0.is_none()
    }

    /// Read the new delimiter from the rest of the `DELIMITER` line starting
    /// at `after`; returns where that line ends. A line without one keeps the
    /// current delimiter.
    pub(super) fn set_from_line(&mut self, source: &str, after: usize) -> usize {
        let rest = source.get(after..).unwrap_or_default();
        let line_end = rest
            .find('\n')
            .map_or(source.len(), |offset| after + offset);
        let line = source.get(after..line_end).unwrap_or_default();
        if let Some(text) = line.split_whitespace().next() {
            self.0 = (text != ";").then(|| text.to_owned());
        }
        line_end
    }

    /// Where the custom delimiter starts inside (or at) a code token.
    pub(super) fn found_at(&self, token: Token, source: &str) -> Option<Range<usize>> {
        let delimiter = self.0.as_deref()?;
        let opaque = matches!(
            token.kind,
            TokenKind::String | TokenKind::QuotedIdent | TokenKind::DollarString
        );
        if opaque || token.kind.is_trivia() {
            return None;
        }
        (token.start..token.end)
            .find(|&index| {
                source
                    .get(index..)
                    .is_some_and(|rest| rest.starts_with(delimiter))
            })
            .map(|start| start..start + delimiter.len())
    }
}
