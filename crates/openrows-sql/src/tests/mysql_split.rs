// SPDX-License-Identifier: Apache-2.0
//! `MySQL` lexing and splitting: backticks, `#` and `-- ` comments,
//! backslash escapes, executable comments, routine bodies and `DELIMITER`.

use super::texts_in;
use crate::{Dialect, split};

const MY: Dialect = Dialect::MySql;

fn assert_split(cases: &[(&str, &[&str])]) {
    for (source, expected) in cases {
        assert_eq!(&texts_in(source, MY), expected, "source: {source:?}");
    }
}

#[test]
fn quotes_and_comments_hide_semicolons() {
    assert_split(&[
        (
            "SELECT `a;b` FROM t; SELECT 2",
            &["SELECT `a;b` FROM t", "SELECT 2"],
        ),
        (
            "SELECT `a``;` FROM t; SELECT 2",
            &["SELECT `a``;` FROM t", "SELECT 2"],
        ),
        (
            "SELECT 'it\\'s; ok'; SELECT 2",
            &["SELECT 'it\\'s; ok'", "SELECT 2"],
        ),
        (
            "SELECT \"a\\\"; b\"; SELECT 2",
            &["SELECT \"a\\\"; b\"", "SELECT 2"],
        ),
        (
            "SELECT 'a''; b'; SELECT 2",
            &["SELECT 'a''; b'", "SELECT 2"],
        ),
        (
            "SELECT 1 # ; no\n; SELECT 2",
            &["SELECT 1 # ; no", "SELECT 2"],
        ),
        (
            "SELECT 1 -- ; no\n; SELECT 2",
            &["SELECT 1 -- ; no", "SELECT 2"],
        ),
        (
            "SELECT 1 /* ; */; SELECT 2",
            &["SELECT 1 /* ; */", "SELECT 2"],
        ),
        ("SELECT $a; SELECT 2", &["SELECT $a", "SELECT 2"]),
    ]);
}

#[test]
fn double_dash_needs_whitespace_to_start_a_comment() {
    assert_split(&[
        ("SELECT 1--1; SELECT 2", &["SELECT 1--1", "SELECT 2"]),
        ("SELECT 5 --1; SELECT 2", &["SELECT 5 --1", "SELECT 2"]),
        ("SELECT 1 --\n; SELECT 2", &["SELECT 1 --", "SELECT 2"]),
    ]);
}

#[test]
fn nested_comment_markers_do_not_nest() {
    assert_split(&[(
        "SELECT /* a /* b */ 1; SELECT 2",
        &["SELECT /* a /* b */ 1", "SELECT 2"],
    )]);
}

#[test]
fn routine_bodies_keep_their_semicolons() {
    let procedure = "CREATE PROCEDURE p() BEGIN DECLARE x INT; IF x THEN SELECT 1; END IF; \
                     WHILE x DO SET x = x - 1; END WHILE; CASE x WHEN 1 THEN SELECT 1; END CASE; \
                     SELECT CASE WHEN x THEN 1 END; END";
    let trigger = "CREATE TRIGGER t BEFORE INSERT ON a FOR EACH ROW BEGIN SET NEW.x = 1; END";
    let event = "CREATE EVENT e ON SCHEDULE EVERY 1 DAY DO BEGIN DELETE FROM a WHERE x; END";
    let anonymous = "BEGIN NOT ATOMIC SELECT 1; SELECT 2; END";
    let source = format!("{procedure}; {trigger};\n{event}; {anonymous}; SELECT 9");
    assert_eq!(
        texts_in(&source, MY),
        vec![procedure, trigger, event, anonymous, "SELECT 9"]
    );
}

#[test]
fn begin_alone_is_a_transaction() {
    assert_split(&[(
        "BEGIN; UPDATE t SET a = 1 WHERE id = 2; COMMIT",
        &["BEGIN", "UPDATE t SET a = 1 WHERE id = 2", "COMMIT"],
    )]);
}

#[test]
fn delimiter_lines_change_the_terminator() {
    let source = "DELIMITER //\nCREATE PROCEDURE p() BEGIN SELECT 1; SELECT 2; END//\n\
                  CALL p()//\nDELIMITER ;\nSELECT 3; SELECT 4";
    assert_eq!(
        texts_in(source, MY),
        vec![
            "CREATE PROCEDURE p() BEGIN SELECT 1; SELECT 2; END",
            "CALL p()",
            "SELECT 3",
            "SELECT 4"
        ]
    );
    let dollar = "delimiter $$\nSELECT 'a$$b'$$ SELECT 1 END$$\ndelimiter ;\nSELECT 2;";
    assert_eq!(
        texts_in(dollar, MY),
        vec!["SELECT 'a$$b'", "SELECT 1 END", "SELECT 2"]
    );
}

#[test]
fn delimiter_terminators_are_located_exactly() {
    let source = "DELIMITER ;;\nSELECT 1;;SELECT 2";
    let statements = split(source, MY);
    assert_eq!(statements.len(), 2);
    let first = &statements[0];
    assert_eq!(first.text(source), "SELECT 1");
    assert_eq!(
        first.terminator.clone().and_then(|range| source.get(range)),
        Some(";;")
    );
    assert_eq!(
        source.get(first.range_with_terminator()),
        Some("SELECT 1;;")
    );
}

#[test]
fn delimiter_is_only_a_command_at_statement_start() {
    assert_split(&[(
        "SELECT delimiter FROM t; SELECT 2",
        &["SELECT delimiter FROM t", "SELECT 2"],
    )]);
}

#[test]
fn postgres_rules_do_not_apply() {
    assert_split(&[
        ("SELECT $$; SELECT 2", &["SELECT $$", "SELECT 2"]),
        (
            "SELECT E'a\\'; b'; SELECT 2",
            &["SELECT E'a\\'; b'", "SELECT 2"],
        ),
    ]);
}
