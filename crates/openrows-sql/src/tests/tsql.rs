// SPDX-License-Identifier: Apache-2.0
//! SQL Server classification, parameters and session context.

use super::check_in;
use crate::{ContextSetting, Dialect, PlaceholderKind, StatementKind, placeholders};

use StatementKind::{Ddl, Read, TxControl, Unknown, Write};

const MS: Dialect = Dialect::SqlServer;

#[test]
fn heads_classify_t_sql() {
    check_in(
        MS,
        &[
            ("SELECT * FROM [dbo].[t]", Read, false),
            ("WITH c AS (SELECT 1 AS a) SELECT a FROM c", Read, false),
            ("PRINT 'hello'", Read, false),
            ("DECLARE @n int = 1", Read, false),
            ("SET @n = (SELECT COUNT(*) FROM t)", Read, false),
            ("SET NOCOUNT ON", Read, false),
            ("SET IDENTITY_INSERT t ON", Unknown, false),
            ("USE shop", Read, false),
            ("WAITFOR DELAY '00:00:01'", Read, false),
            ("EXEC sp_help 't'", Read, false),
            ("EXEC sys.sp_rename N'dbo.t.a', N'b', N'COLUMN'", Ddl, false),
            ("EXEC dbo.do_things", Unknown, false),
            ("EXEC (@sql)", Unknown, false),
            ("INSERT INTO t (a) VALUES (1)", Write, false),
            ("UPDATE t SET a = 1 WHERE id = 2", Write, false),
            (
                "MERGE t AS d USING s ON d.id = s.id WHEN MATCHED THEN DELETE;",
                Write,
                false,
            ),
            ("BEGIN TRAN", TxControl, false),
            ("BEGIN TRANSACTION t1", TxControl, false),
            ("COMMIT", TxControl, false),
            ("SAVE TRANSACTION s1", TxControl, false),
            ("SET TRANSACTION ISOLATION LEVEL SNAPSHOT", TxControl, false),
            (
                "BEGIN TRY SELECT 1 END TRY BEGIN CATCH SELECT 2 END CATCH",
                Unknown,
                false,
            ),
            ("CREATE TABLE t (id int)", Ddl, false),
            ("DBCC USEROPTIONS", Read, false),
            ("DBCC FREEPROCCACHE", Write, false),
            ("KILL 53", Write, false),
            ("IF 1 = 1 SELECT 1", Unknown, false),
        ],
    );
}

#[test]
fn batches_cannot_hide_writes_behind_a_read() {
    check_in(
        MS,
        &[
            ("SELECT 1\nDELETE FROM t WHERE id = 1", Unknown, false),
            ("SELECT 1 UPDATE t SET a = 1 WHERE b = 2", Unknown, false),
            ("SELECT a INTO #copy FROM t", Write, false),
            ("SELECT NEXT VALUE FOR dbo.seq", Unknown, false),
            ("PRINT 1 EXEC dbo.p", Unknown, false),
            ("SELECT [delete], [update] FROM t", Read, false),
            ("BEGIN TRAN DROP TABLE t", Unknown, true),
        ],
    );
}

#[test]
fn unbounded_or_destructive_statements_need_confirmation() {
    check_in(
        MS,
        &[
            ("DELETE FROM t", Write, true),
            ("DELETE t", Write, true),
            ("UPDATE t SET a = 1", Write, true),
            ("UPDATE TOP (5) t SET a = 1", Write, true),
            ("DELETE FROM t\nSELECT * FROM u WHERE id = 1", Write, true),
            (
                "UPDATE t SET a = CASE WHEN b = 1 THEN 2 END WHERE id = 3",
                Write,
                false,
            ),
            ("TRUNCATE TABLE t", Write, true),
            ("DROP TABLE t", Ddl, true),
            ("RESTORE DATABASE d FROM DISK = 'x.bak'", Write, true),
            ("SHUTDOWN WITH NOWAIT", Write, true),
            ("ALTER TABLE t DROP COLUMN c", Ddl, true),
            (
                "CREATE TABLE c (p int REFERENCES p ON DELETE CASCADE ON UPDATE NO ACTION)",
                Ddl,
                false,
            ),
            ("GRANT UPDATE ON t TO app", Ddl, false),
            ("UPDATE STATISTICS t", Write, false),
        ],
    );
}

#[test]
fn variables_and_temp_tables_are_not_parameters() {
    let sql = "SELECT @a, @@VERSION, #t.x, N'?:x', [?] FROM #t WHERE id = :id AND n = ?";
    let found: Vec<_> = placeholders(sql, MS)
        .into_iter()
        .map(|placeholder| (placeholder.kind, &sql[placeholder.range]))
        .collect();
    assert_eq!(
        found,
        [
            (PlaceholderKind::Named, ":id"),
            (PlaceholderKind::Anonymous, "?")
        ]
    );
    assert!(placeholders("SELECT geography::Point(1, 2, 4326)", MS).is_empty());
}

#[test]
fn use_is_replayed() {
    let ms = |sql| crate::session_context(sql, MS);
    assert_eq!(ms("USE shop"), Some(ContextSetting::Database));
    assert_eq!(ms("use [order items]"), Some(ContextSetting::Database));
    assert_eq!(ms("USE shop; DROP TABLE t"), None);
}
