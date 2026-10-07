// SPDX-License-Identifier: Apache-2.0
//! Property tests: splitting is lossless and total on arbitrary input.

use proptest::prelude::*;

use super::PG;
use crate::lexer::{Lexer, TokenKind};
use crate::{classify, placeholders, split, statement_at};

/// SQL-ish text dense in quoting and comment characters.
fn sqlish() -> impl Strategy<Value = String> {
    proptest::string::string_regex(
        "([-;'\"$/*()E\\\\ \nab1é]|\\$\\$|\\$t\\$|--|/\\*|\\*/|BEGIN ATOMIC|END|CASE){0,40}",
    )
    .unwrap()
}

/// The text between statements must hold only whitespace, comments and `;`.
fn gap_is_separator(gap: &str) -> bool {
    Lexer::new(gap).all(|token| token.kind.is_trivia() || token.kind == TokenKind::Semicolon)
}

fn check_lossless(source: &str) -> Result<(), TestCaseError> {
    let statements = split(source, PG);
    let mut cursor = 0;
    let mut rebuilt = String::new();
    for statement in &statements {
        let (start, end) = (statement.range.start, statement.range.end);
        prop_assert!(cursor <= start && start < end && end <= source.len());
        let gap = source.get(cursor..start);
        prop_assert!(gap.is_some_and(gap_is_separator), "gap {gap:?}");
        rebuilt.push_str(gap.unwrap_or_default());
        rebuilt.push_str(statement.text(source));
        prop_assert_eq!(split(statement.text(source), PG).len(), 1);
        cursor = end;
    }
    let tail = source.get(cursor..);
    prop_assert!(tail.is_some_and(gap_is_separator), "tail {tail:?}");
    rebuilt.push_str(tail.unwrap_or_default());
    prop_assert_eq!(rebuilt, source);
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 2048, rng_seed: proptest::test_runner::RngSeed::Fixed(7), ..ProptestConfig::default() })]

    #[test]
    fn split_then_join_preserves_text(source in sqlish()) {
        check_lossless(&source)?;
    }

    #[test]
    fn arbitrary_unicode_never_panics(source in any::<String>(), cursor in any::<usize>()) {
        check_lossless(&source)?;
        let statements = split(&source, PG);
        prop_assert_eq!(statement_at(&statements, cursor).is_none(), statements.is_empty());
        for statement in &statements {
            let _ = classify(statement.text(&source), PG);
        }
        for placeholder in placeholders(&source, PG) {
            prop_assert!(source.get(placeholder.range).is_some());
        }
    }

    #[test]
    fn lexer_covers_every_byte(source in sqlish()) {
        let mut end = 0;
        for token in Lexer::new(&source) {
            prop_assert_eq!(token.start, end);
            prop_assert!(token.end > token.start && source.is_char_boundary(token.end));
            end = token.end;
        }
        prop_assert_eq!(end, source.len());
    }
}
