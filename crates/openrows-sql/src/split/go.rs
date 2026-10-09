// SPDX-License-Identifier: Apache-2.0
//! The SQL Server client tools' `GO` batch separator: a line holding only
//! `GO`, optionally a repeat count and a `--` comment.

use std::ops::Range;

/// The range of `GO` and its count when the word `GO` at `start..end`
/// stands alone on its line; `None` when anything else shares the line.
pub(super) fn separator_line(source: &str, start: usize, end: usize) -> Option<Range<usize>> {
    let line_start = source
        .get(..start)?
        .rfind('\n')
        .map_or(0, |newline| newline + 1);
    if !source.get(line_start..start)?.trim().is_empty() {
        return None;
    }
    let rest = source.get(end..)?;
    let line = rest.split('\n').next().unwrap_or_default();
    let code = line.split("--").next().unwrap_or_default();
    let count = code.trim();
    if !count.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let count_end = if count.is_empty() {
        end
    } else {
        end + code.find(count).unwrap_or(0) + count.len()
    };
    Some(start..count_end)
}

#[cfg(test)]
mod tests {
    use super::separator_line;

    fn at(source: &str) -> Option<&str> {
        let start = source.find("GO").or_else(|| source.find("go"))?;
        separator_line(source, start, start + 2).and_then(|range| source.get(range))
    }

    #[test]
    fn go_alone_on_its_line_separates() {
        assert_eq!(at("SELECT 1\nGO\nSELECT 2"), Some("GO"));
        assert_eq!(at("SELECT 1\n  go  \n"), Some("go"));
        assert_eq!(at("SELECT 1\nGO 3 -- thrice\n"), Some("GO 3"));
        assert_eq!(at("GO"), Some("GO"));
        assert_eq!(at("SELECT 1\r\nGO\r\n"), Some("GO"));
    }

    #[test]
    fn go_sharing_its_line_does_not() {
        assert_eq!(at("SELECT 1 GO\n"), None);
        assert_eq!(at("SELECT 1\nGO SELECT 2\n"), None);
        assert_eq!(at("SELECT 1\nGO;\n"), None);
    }
}
