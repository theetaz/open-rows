// SPDX-License-Identifier: Apache-2.0
//! SQL Server lexing and splitting: `[…]` identifiers, `N'…'` strings,
//! nested comments, `BEGIN … END` blocks, module bodies and `GO` lines.

use super::texts_in;
use crate::{Dialect, split};

const MS: Dialect = Dialect::SqlServer;

fn assert_split(cases: &[(&str, &[&str])]) {
    for (source, expected) in cases {
        assert_eq!(&texts_in(source, MS), expected, "source: {source:?}");
    }
}

#[test]
fn quotes_and_comments_hide_semicolons_and_go() {
    assert_split(&[
        (
            "SELECT [a;b] FROM t; SELECT 2",
            &["SELECT [a;b] FROM t", "SELECT 2"],
        ),
        (
            "SELECT [a]];b] FROM t; SELECT 2",
            &["SELECT [a]];b] FROM t", "SELECT 2"],
        ),
        (
            "SELECT N'it''s; ok'; SELECT 2",
            &["SELECT N'it''s; ok'", "SELECT 2"],
        ),
        ("SELECT 'a\\'; SELECT 2", &["SELECT 'a\\'", "SELECT 2"]),
        (
            "SELECT 1 /* outer /* inner; */ still; */; SELECT 2",
            &["SELECT 1 /* outer /* inner; */ still; */", "SELECT 2"],
        ),
        (
            "SELECT 'x\nGO\ny'; SELECT 2",
            &["SELECT 'x\nGO\ny'", "SELECT 2"],
        ),
        (
            "SELECT 1 /*\nGO\n*/; SELECT 2",
            &["SELECT 1 /*\nGO\n*/", "SELECT 2"],
        ),
    ]);
}

#[test]
fn go_lines_end_batches() {
    let source = "SELECT 1\nSELECT 2\nGO\nSELECT 3\n  go 2 -- twice\nGO\n";
    let statements = split(source, MS);
    let texts: Vec<_> = statements.iter().map(|s| s.text(source)).collect();
    assert_eq!(texts, ["SELECT 1\nSELECT 2", "SELECT 3"]);
    let first = &statements[0];
    assert_eq!(
        first.terminator.clone().and_then(|range| source.get(range)),
        Some("GO")
    );
    let second = &statements[1];
    assert_eq!(
        second
            .terminator
            .clone()
            .and_then(|range| source.get(range)),
        Some("go 2")
    );
    assert_split(&[
        ("SELECT go FROM t GO", &["SELECT go FROM t GO"]),
        ("SELECT 1\nGO;\nSELECT 2", &["SELECT 1\nGO", "SELECT 2"]),
        ("GO\nGO\nSELECT 1\nGO", &["SELECT 1"]),
    ]);
}

#[test]
fn blocks_keep_their_semicolons() {
    assert_split(&[
        (
            "IF 1 = 1 BEGIN SELECT 1; SELECT 2; END; SELECT 3",
            &["IF 1 = 1 BEGIN SELECT 1; SELECT 2; END", "SELECT 3"],
        ),
        (
            "BEGIN TRY SELECT 1; END TRY BEGIN CATCH SELECT 2; END CATCH; SELECT 3",
            &[
                "BEGIN TRY SELECT 1; END TRY BEGIN CATCH SELECT 2; END CATCH",
                "SELECT 3",
            ],
        ),
        (
            "BEGIN TRAN; UPDATE t SET a = 1; COMMIT;",
            &["BEGIN TRAN", "UPDATE t SET a = 1", "COMMIT"],
        ),
        (
            "SELECT CASE WHEN a = 1 THEN 'x' END AS c FROM t; SELECT 2",
            &[
                "SELECT CASE WHEN a = 1 THEN 'x' END AS c FROM t",
                "SELECT 2",
            ],
        ),
        (
            "BEGIN SELECT CASE WHEN 1 = 1 THEN 1 END; END CONVERSATION @h; END; SELECT 2",
            &[
                "BEGIN SELECT CASE WHEN 1 = 1 THEN 1 END; END CONVERSATION @h; END",
                "SELECT 2",
            ],
        ),
    ]);
}

#[test]
fn module_bodies_run_to_the_end_of_the_batch() {
    let procedure = "CREATE PROCEDURE dbo.p AS\nSELECT 1;\nSELECT 2;";
    assert_split(&[
        (procedure, &[procedure]),
        (
            "CREATE OR ALTER VIEW v AS SELECT 1 AS a;\nGO\nSELECT * FROM v;",
            &[
                "CREATE OR ALTER VIEW v AS SELECT 1 AS a;",
                "SELECT * FROM v",
            ],
        ),
        (
            "ALTER TRIGGER tr ON t AFTER INSERT AS BEGIN SET NOCOUNT ON; END;\nGO\nSELECT 1",
            &[
                "ALTER TRIGGER tr ON t AFTER INSERT AS BEGIN SET NOCOUNT ON; END;",
                "SELECT 1",
            ],
        ),
        (
            "CREATE TABLE view_log (proc_id int); SELECT 1",
            &["CREATE TABLE view_log (proc_id int)", "SELECT 1"],
        ),
    ]);
}
