// SPDX-License-Identifier: Apache-2.0
//! `SQLite` lexing, splitting and safety classification.

use super::{check_in, texts_in};
use crate::{Dialect, PlaceholderKind, StatementKind, placeholders};

use StatementKind::{Ddl as D, Read as R, TxControl as T, Unknown as U, Write as W};

const LITE: Dialect = Dialect::Sqlite;

#[test]
fn identifiers_strings_and_comments_hide_semicolons() {
    let cases: &[(&str, &[&str])] = &[
        (
            "SELECT [a;b] FROM t; SELECT 2",
            &["SELECT [a;b] FROM t", "SELECT 2"],
        ),
        (
            "SELECT `a;b` FROM t; SELECT 2",
            &["SELECT `a;b` FROM t", "SELECT 2"],
        ),
        (
            "SELECT \"a;\"\"b\"; SELECT 2",
            &["SELECT \"a;\"\"b\"", "SELECT 2"],
        ),
        (
            "SELECT 'it''s; ok'; SELECT 2",
            &["SELECT 'it''s; ok'", "SELECT 2"],
        ),
        ("SELECT 'a\\'; SELECT 2", &["SELECT 'a\\'", "SELECT 2"]),
        ("SELECT 1 -- ;\n; SELECT 2", &["SELECT 1 -- ;", "SELECT 2"]),
        ("SELECT x'00'; SELECT 2", &["SELECT x'00'", "SELECT 2"]),
    ];
    for (source, expected) in cases {
        assert_eq!(&texts_in(source, LITE), expected, "source: {source:?}");
    }
}

#[test]
fn trigger_bodies_keep_their_semicolons() {
    let trigger = "CREATE TEMP TRIGGER t AFTER INSERT ON a BEGIN \
                   UPDATE b SET n = CASE WHEN new.x THEN 1 END WHERE id = new.id; \
                   DELETE FROM c WHERE id = new.id; END";
    let source = format!("{trigger}; BEGIN; SELECT 1; END;");
    assert_eq!(
        texts_in(&source, LITE),
        vec![trigger, "BEGIN", "SELECT 1", "END"]
    );
}

#[test]
fn parameters() {
    let source = "SELECT ?, ?2, :name, $tcl, @at FROM t WHERE a = '?'";
    let kinds: Vec<_> = placeholders(source, LITE)
        .into_iter()
        .map(|placeholder| (placeholder.kind, &source[placeholder.range]))
        .collect();
    assert_eq!(
        kinds,
        vec![
            (PlaceholderKind::Anonymous, "?"),
            (PlaceholderKind::Positional, "?2"),
            (PlaceholderKind::Named, ":name"),
            (PlaceholderKind::Named, "$tcl"),
        ]
    );
}

#[test]
fn classification() {
    check_in(
        LITE,
        &[
            ("SELECT * FROM [delete]", R, false),
            ("VALUES (1)", R, false),
            ("EXPLAIN QUERY PLAN DELETE FROM t", R, false),
            ("EXPLAIN DELETE FROM t", R, false),
            ("PRAGMA table_info(t)", R, false),
            ("PRAGMA main.table_info('t')", R, false),
            ("PRAGMA user_version", R, false),
            ("PRAGMA journal_mode", R, false),
            ("PRAGMA integrity_check", R, false),
            ("PRAGMA journal_mode = WAL", U, false),
            ("PRAGMA journal_mode(WAL)", U, false),
            ("PRAGMA writable_schema = 1", U, false),
            ("PRAGMA optimize", U, false),
            ("PRAGMA wal_checkpoint", U, false),
            ("INSERT INTO t VALUES (1)", W, false),
            ("INSERT OR REPLACE INTO t VALUES (1)", W, false),
            ("REPLACE INTO t VALUES (1)", W, false),
            (
                "INSERT INTO t VALUES (1) ON CONFLICT DO UPDATE SET a = 2",
                W,
                false,
            ),
            ("UPDATE t SET a = 1 WHERE rowid = 2", W, false),
            ("DELETE FROM t WHERE id = 1", W, false),
            ("SELECT load_extension('x')", W, false),
            ("ATTACH 'other.db' AS other", W, false),
            ("DETACH other", R, false),
            ("VACUUM", W, false),
            ("VACUUM INTO 'copy.db'", W, false),
            ("WITH x AS (SELECT 1) DELETE FROM t WHERE id IN x", W, false),
            (
                "WITH x AS (SELECT 1) REPLACE INTO t SELECT * FROM x",
                W,
                false,
            ),
            ("CREATE TABLE t (id INTEGER PRIMARY KEY)", D, false),
            ("ALTER TABLE t RENAME TO u", D, false),
            ("BEGIN IMMEDIATE", T, false),
            ("COMMIT", T, false),
            ("END TRANSACTION", T, false),
            ("SAVEPOINT s", T, false),
            ("SELECT 1; DELETE FROM t", W, true),
        ],
    );
}

#[test]
fn dangerous_statements_need_confirmation() {
    check_in(
        LITE,
        &[
            ("DELETE FROM t", W, true),
            ("UPDATE t SET a = 1", W, true),
            ("DROP TABLE t", D, true),
            ("DROP INDEX i", D, true),
            ("ALTER TABLE t DROP COLUMN a", D, true),
            ("WITH x AS (SELECT 1) DELETE FROM t", W, true),
        ],
    );
}
