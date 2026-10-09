// SPDX-License-Identifier: Apache-2.0
//! Small SHOW GRANTS lexer preserving quoted server names verbatim.
use openrows_driver_api::DriverError;

/// Tokenize punctuation and words while keeping quoted names intact.
pub(super) fn tokens(sql: &str) -> Result<Vec<&str>, DriverError> {
    let mut found = Vec::new();
    let mut chars = sql.char_indices().peekable();
    while let Some((start, ch)) = chars.next() {
        if ch.is_whitespace() {
            continue;
        }
        let end = if matches!(ch, '\'' | '`' | '"') {
            quoted_end(&mut chars, ch)?
        } else if ",().@*;".contains(ch) {
            start + ch.len_utf8()
        } else {
            word_end(&mut chars, start + ch.len_utf8())
        };
        found.push(&sql[start..end]);
    }
    Ok(found)
}

fn word_end(chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>, mut end: usize) -> usize {
    while let Some(&(index, next)) = chars.peek() {
        if next.is_whitespace() || ",().@*;'`\"".contains(next) {
            break;
        }
        chars.next();
        end = index + next.len_utf8();
    }
    end
}

fn quoted_end(
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
    quote: char,
) -> Result<usize, DriverError> {
    while let Some((index, ch)) = chars.next() {
        if ch == '\\' && quote != '`' {
            chars.next();
        } else if ch == quote {
            if chars.peek().is_some_and(|(_, next)| *next == quote) {
                chars.next();
            } else {
                return Ok(index + ch.len_utf8());
            }
        }
    }
    Err(invalid())
}

/// Split a privilege/role/column list on commas outside parentheses.
pub(super) fn groups<'a>(tokens: &'a [&'a str]) -> Result<Vec<&'a [&'a str]>, DriverError> {
    let (mut depth, mut start) = (0_u32, 0);
    let mut groups = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        match *token {
            "(" => depth += 1,
            ")" => depth = depth.checked_sub(1).ok_or_else(invalid)?,
            "," if depth == 0 => {
                groups.push(&tokens[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    if depth != 0 {
        return Err(invalid());
    }
    groups.push(&tokens[start..]);
    Ok(groups)
}

/// Malformed server grants remain visible as a typed error.
pub(super) fn invalid() -> DriverError {
    DriverError::Protocol("unrecognized SHOW GRANTS row".to_owned())
}
