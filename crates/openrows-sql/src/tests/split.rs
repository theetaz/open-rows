// SPDX-License-Identifier: Apache-2.0

use super::{PG, texts};
use crate::{Statement, split, statement_at};

#[test]
fn splits_on_semicolons_outside_quotes_and_comments() {
    let cases: &[(&str, &[&str])] = &[
        ("SELECT 1; SELECT 2", &["SELECT 1", "SELECT 2"]),
        ("SELECT 1;SELECT 2;", &["SELECT 1", "SELECT 2"]),
        ("SELECT ';'; SELECT 2", &["SELECT ';'", "SELECT 2"]),
        (
            "SELECT 'it''s; ok'; SELECT 2",
            &["SELECT 'it''s; ok'", "SELECT 2"],
        ),
        ("SELECT 'a\\'; SELECT 2", &["SELECT 'a\\'", "SELECT 2"]),
        (
            "SELECT E'a\\'; b'; SELECT 2",
            &["SELECT E'a\\'; b'", "SELECT 2"],
        ),
        ("select e'\\\\'; select 2", &["select e'\\\\'", "select 2"]),
        (
            "SELECT E'it''s;'; SELECT 2",
            &["SELECT E'it''s;'", "SELECT 2"],
        ),
        (
            "SELECT \"a;b\" FROM t; SELECT 2",
            &["SELECT \"a;b\" FROM t", "SELECT 2"],
        ),
        (
            "SELECT \"a\"\";\"; SELECT 2",
            &["SELECT \"a\"\";\"", "SELECT 2"],
        ),
        (
            "SELECT 1 -- ; no\n; SELECT 2",
            &["SELECT 1 -- ; no", "SELECT 2"],
        ),
        (
            "SELECT /* a /* ; */ ; */ 1; SELECT 2",
            &["SELECT /* a /* ; */ ; */ 1", "SELECT 2"],
        ),
        (
            "SELECT 1 /* c */ ; SELECT 2",
            &["SELECT 1 /* c */", "SELECT 2"],
        ),
        (
            "SELECT 'héllo;'; SELECT ünï; SELECT 3",
            &["SELECT 'héllo;'", "SELECT ünï", "SELECT 3"],
        ),
        ("SELECT 1;\r\nSELECT 2\r\n", &["SELECT 1", "SELECT 2"]),
        ("SELECT a$b; SELECT 2", &["SELECT a$b", "SELECT 2"]),
        ("SELECT 1-2; SELECT 2", &["SELECT 1-2", "SELECT 2"]),
    ];
    for (source, expected) in cases {
        assert_eq!(&texts(source), expected, "source: {source:?}");
    }
}

#[test]
fn dollar_quotes_hide_semicolons() {
    let cases: &[(&str, &[&str])] = &[
        (
            "CREATE FUNCTION f() RETURNS int AS $$ SELECT 1; $$ LANGUAGE sql; SELECT 2",
            &[
                "CREATE FUNCTION f() RETURNS int AS $$ SELECT 1; $$ LANGUAGE sql",
                "SELECT 2",
            ],
        ),
        (
            "DO $body$ BEGIN PERFORM $$x;$$; END $body$; SELECT 2",
            &["DO $body$ BEGIN PERFORM $$x;$$; END $body$", "SELECT 2"],
        ),
        (
            "SELECT $a$ ; $b$ ; $a$; SELECT 2",
            &["SELECT $a$ ; $b$ ; $a$", "SELECT 2"],
        ),
        (
            "SELECT $_x1$;$_x1$; SELECT 2",
            &["SELECT $_x1$;$_x1$", "SELECT 2"],
        ),
        ("SELECT $1; SELECT $2", &["SELECT $1", "SELECT $2"]),
        ("SELECT $1$; SELECT 2", &["SELECT $1$", "SELECT 2"]),
        (
            "SELECT 'a' $ 'b'; SELECT 2",
            &["SELECT 'a' $ 'b'", "SELECT 2"],
        ),
    ];
    for (source, expected) in cases {
        assert_eq!(&texts(source), expected, "source: {source:?}");
    }
}

#[test]
fn nesting_keeps_semicolons_inside_statement() {
    let rule = "CREATE RULE r AS ON INSERT TO t DO ALSO (INSERT INTO a VALUES (1); INSERT INTO b VALUES (2))";
    let atomic = "CREATE FUNCTION f() RETURNS int LANGUAGE sql BEGIN ATOMIC SELECT CASE WHEN true THEN 1 END; SELECT 2; END";
    let cases: &[(String, Vec<&str>)] = &[
        (format!("{rule}; SELECT 1"), vec![rule, "SELECT 1"]),
        (format!("{atomic}; SELECT 3"), vec![atomic, "SELECT 3"]),
        (
            "BEGIN; SELECT 1; END; COMMIT".to_owned(),
            vec!["BEGIN", "SELECT 1", "END", "COMMIT"],
        ),
        (
            "SELECT (1; SELECT 2".to_owned(),
            vec!["SELECT (1; SELECT 2"],
        ),
        (
            "SELECT 1); SELECT 2".to_owned(),
            vec!["SELECT 1)", "SELECT 2"],
        ),
    ];
    for (source, expected) in cases {
        assert_eq!(&texts(source), expected, "source: {source:?}");
    }
}

#[test]
fn drops_empty_and_comment_only_statements() {
    let cases: &[(&str, &[&str])] = &[
        ("", &[]),
        ("   \n\t", &[]),
        (";;;", &[]),
        ("-- only a comment", &[]),
        ("/* a */ ; -- b\n;", &[]),
        ("-- only\n; /* x */ ;  ; SELECT 1", &["SELECT 1"]),
        (
            "SELECT 1; -- c1\n-- c2\nSELECT 2",
            &["SELECT 1", "-- c2\nSELECT 2"],
        ),
        ("-- header\nSELECT 1", &["-- header\nSELECT 1"]),
        ("SELECT 1; /* trailing */", &["SELECT 1"]),
    ];
    for (source, expected) in cases {
        assert_eq!(&texts(source), expected, "source: {source:?}");
    }
}

#[test]
fn unterminated_constructs_extend_to_end() {
    let cases: &[(&str, &[&str])] = &[
        ("SELECT 'abc; SELECT 2", &["SELECT 'abc; SELECT 2"]),
        ("SELECT \"abc; SELECT 2", &["SELECT \"abc; SELECT 2"]),
        ("SELECT $$abc; SELECT 2", &["SELECT $$abc; SELECT 2"]),
        ("SELECT E'abc\\", &["SELECT E'abc\\"]),
        ("SELECT 1; /* ; SELECT 2", &["SELECT 1"]),
        ("SELECT 1;\n/* /* */ ; SELECT 2", &["SELECT 1"]),
        (
            "SELECT 1 /* /* */ ; SELECT 2",
            &["SELECT 1 /* /* */ ; SELECT 2"],
        ),
    ];
    for (source, expected) in cases {
        assert_eq!(&texts(source), expected, "source: {source:?}");
    }
}

#[test]
fn keeps_byte_offsets_and_terminators() {
    let source = "  SELECT 1 ;\nSELECT 'é'";
    let expected = vec![
        Statement {
            range: 2..10,
            terminator: Some(11..12),
        },
        Statement {
            range: 13..source.len(),
            terminator: None,
        },
    ];
    let statements = split(source, PG);
    assert_eq!(statements, expected);
    assert_eq!(statements[0].range_with_terminator(), 2..12);
    assert_eq!(statements[1].range_with_terminator(), 13..source.len());
}

#[test]
fn statement_at_picks_containing_then_preceding() {
    let source = "SELECT 1;\n\nSELECT 2; -- t\n-- lead\nSELECT 3";
    let statements = split(source, PG);
    assert_eq!(statements.len(), 3);
    let at = |needle: &str, delta: usize| source.find(needle).map(|offset| offset + delta);
    let cases: &[(Option<usize>, usize)] = &[
        (Some(0), 0),
        (Some(4), 0),
        (at(";", 1), 0),
        (at("\n\n", 1), 0),
        (at("SELECT 2", 0), 1),
        (at("-- t", 2), 1),
        (at("-- lead", 0), 2),
        (Some(source.len()), 2),
        (Some(usize::MAX), 2),
    ];
    for &(cursor, expected) in cases {
        let cursor = cursor.unwrap_or_default();
        assert_eq!(
            statement_at(&statements, cursor),
            Some(expected),
            "cursor {cursor}"
        );
    }
}

#[test]
fn statement_at_handles_edges() {
    assert_eq!(statement_at(&[], 0), None);
    let leading = "\n\n  SELECT 1";
    assert_eq!(statement_at(&split(leading, PG), 0), Some(0));
    let adjacent = "SELECT 1;SELECT 2";
    assert_eq!(statement_at(&split(adjacent, PG), 9), Some(1));
}
