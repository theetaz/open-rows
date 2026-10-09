// SPDX-License-Identifier: Apache-2.0
//! The streamed splitter finds exactly the statements of `split`, wherever
//! the chunks are cut.
use crate::{Dialect, ScriptStatement, StreamSplitter, split};

const PG_SCRIPT: &str = "-- header; comment\nCREATE TABLE t (a int, b text);\n\
    INSERT INTO t VALUES (1, 'x;y'), (2, $$dollar; quoted$$); -- trailing\n\
    /* block; comment */ SELECT 'ünï;😀' ;\n\n\
    CREATE FUNCTION f() RETURNS int LANGUAGE sql BEGIN ATOMIC SELECT 1; SELECT 2; END;\n\
    SELECT 3";

const MYSQL_SCRIPT: &str = "SELECT 1;\nDELIMITER //\n\
    CREATE PROCEDURE p() BEGIN SELECT 'a;b'; SELECT 2; END//\n\
    SELECT 4//\nDELIMITER ;\nSELECT 5; # done\nSELECT `x;y`";

const SQLITE_SCRIPT: &str = "CREATE TABLE t (a);\n\
    CREATE TRIGGER g AFTER INSERT ON t BEGIN\n  UPDATE t SET a = 1; SELECT 1;\nEND;\n\
    INSERT INTO t VALUES ([x;y]);;\n";

/// Feed `script` in chunks of at most `size` bytes, cut on char boundaries.
fn streamed(script: &str, dialect: Dialect, size: usize) -> Vec<ScriptStatement> {
    let mut splitter = StreamSplitter::new(dialect);
    let mut found = Vec::new();
    let mut rest = script;
    while !rest.is_empty() {
        let mut cut = size.min(rest.len());
        while !rest.is_char_boundary(cut) {
            cut += 1;
        }
        let (chunk, after) = rest.split_at(cut);
        found.extend(splitter.push(chunk));
        rest = after;
    }
    found.extend(splitter.finish());
    assert_eq!(splitter.consumed(), script.len() as u64);
    found
}

/// A line holding code, not only a comment or whitespace.
fn is_code(line: &str) -> bool {
    let line = line.trim_start();
    !line.is_empty() && !line.starts_with("--") && !line.starts_with('#')
}

fn line_of(script: &str, offset: usize) -> usize {
    script[..offset].matches('\n').count() + 1
}

#[test]
fn any_chunking_yields_the_statements_of_split() {
    for (script, dialect) in [
        (PG_SCRIPT, Dialect::Postgres),
        (MYSQL_SCRIPT, Dialect::MySql),
        (SQLITE_SCRIPT, Dialect::Sqlite),
    ] {
        let whole = split(script, dialect);
        for size in [1, 2, 3, 5, 8, 13, 64, 4096] {
            let found = streamed(script, dialect, size);
            let texts: Vec<&str> = found.iter().map(|s| s.text.as_str()).collect();
            let expected: Vec<&str> = whole.iter().map(|s| s.text(script)).collect();
            assert_eq!(texts, expected, "{dialect:?} in chunks of {size}");
            for (streamed, statement) in found.iter().zip(&whole) {
                assert_eq!(streamed.offset, statement.range.start as u64);
                assert_eq!(streamed.end, statement.range_with_terminator().end as u64);
                let code = statement
                    .text(script)
                    .lines()
                    .position(is_code)
                    .unwrap_or(0);
                assert_eq!(streamed.line, line_of(script, statement.range.start) + code);
            }
        }
    }
}

#[test]
fn a_custom_delimiter_stays_in_force_across_chunks() {
    let found = streamed(MYSQL_SCRIPT, Dialect::MySql, 3);
    let texts: Vec<&str> = found.iter().map(|s| s.text.as_str()).collect();
    assert_eq!(
        texts,
        [
            "SELECT 1",
            "CREATE PROCEDURE p() BEGIN SELECT 'a;b'; SELECT 2; END",
            "SELECT 4",
            "SELECT 5",
            "SELECT `x;y`"
        ]
    );
    assert_eq!(
        found.iter().map(|s| s.line).collect::<Vec<_>>(),
        [1, 3, 4, 6, 7]
    );
}

#[test]
fn only_the_unfinished_tail_is_kept() {
    let statement = "INSERT INTO t VALUES (1);\n";
    let mut splitter = StreamSplitter::new(Dialect::Postgres);
    let mut count = 0;
    for _ in 0..10_000 {
        count += splitter.push(statement).len();
    }
    count += splitter.finish().len();
    assert_eq!(count, 10_000);
    let mut huge = StreamSplitter::new(Dialect::Postgres);
    assert!(huge.push("INSERT INTO t VALUES ('").is_empty());
    for _ in 0..1_000 {
        assert!(huge.push(&"x".repeat(1_000)).is_empty());
    }
    // Tails are split again only after doubling: the end may wait for `finish`.
    let mut last = huge.push("');SELECT 1");
    last.extend(huge.finish());
    assert_eq!(last.len(), 2);
    assert_eq!(
        last[0].text.len(),
        "INSERT INTO t VALUES ('')".len() + 1_000_000
    );
    assert_eq!(last[1].text, "SELECT 1");
}
