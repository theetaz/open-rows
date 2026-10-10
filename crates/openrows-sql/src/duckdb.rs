// SPDX-License-Identifier: Apache-2.0
//! Conservative attachment admission until every attached file can be reserved.
use crate::{Dialect, lexer::Lexer};

/// Refuse attachment SQL before dispatch, including prepared attachment statements.
/// Strings, quoted identifiers and comments do not count as SQL keywords.
#[must_use]
pub fn has_attachment(sql: &str) -> bool {
    Lexer::new(sql, Dialect::Duckdb).any(|token| token.is_word(sql, "ATTACH"))
}

/// Whether a statement explicitly requests DML result rows.
#[must_use]
pub fn has_returning(sql: &str) -> bool {
    Lexer::new(sql, Dialect::Duckdb).any(|token| token.is_word(sql, "RETURNING"))
}

#[cfg(test)]
mod tests {
    use super::has_attachment;

    #[test]
    fn attachment_detection_respects_quoting_and_comments() {
        for sql in [
            "ATTACH 'a.duckdb' AS a",
            "SELECT 1; /* x */ AtTaCh 'a'",
            "PREPARE a AS ATTACH 'a'",
        ] {
            assert!(has_attachment(sql), "{sql}");
        }
        for sql in [
            "SELECT 'ATTACH', $$ATTACH$$, \"ATTACH\"",
            "-- ATTACH\nSELECT 1",
            "/* ATTACH */ SELECT 1",
        ] {
            assert!(!has_attachment(sql), "{sql}");
        }
    }
}
