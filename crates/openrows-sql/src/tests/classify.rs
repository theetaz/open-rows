// SPDX-License-Identifier: Apache-2.0

use super::check;
use crate::StatementKind;

use StatementKind::{Ddl as D, Read as R, Unknown as U, Write as W};

#[test]
fn reads() {
    check(&[
        ("SELECT 1", R, false),
        ("  -- note\n /* c */ select * from t", R, false),
        ("(SELECT 1) UNION (SELECT 2)", R, false),
        ("VALUES (1), (2)", R, false),
        ("TABLE users", R, false),
        ("SHOW search_path", R, false),
        ("FETCH 10 FROM c", R, false),
        ("SELECT 'DELETE FROM t'", R, false),
        (
            "SELECT \"update\" FROM t WHERE note = 'drop table'",
            R,
            false,
        ),
        ("SELECT $$; DELETE FROM t$$", R, false),
        ("SELECT * FROM t FOR", R, false),
        ("SELECT count(*) FROM t -- DELETE FROM t", R, false),
    ]);
}

#[test]
fn reads_that_write() {
    check(&[
        ("SELECT * INTO new_t FROM t", W, false),
        ("SELECT * FROM t FOR UPDATE", W, false),
        ("SELECT * FROM t FOR NO KEY UPDATE SKIP LOCKED", W, false),
        ("SELECT * FROM t FOR SHARE", W, false),
        ("SELECT * FROM t FOR KEY SHARE", W, false),
        ("SELECT * FROM (SELECT * FROM t FOR UPDATE) s", W, false),
        ("SELECT nextval('s')", W, false),
        ("SELECT pg_catalog.setval('s', 1)", W, false),
        (
            "SELECT set_config('transaction_read_only', 'off', false)",
            W,
            false,
        ),
        ("SELECT pg_terminate_backend(42)", W, false),
        ("SELECT pg_advisory_lock(1)", W, false),
        ("SELECT lo_unlink(1)", W, false),
        ("SELECT dblink_exec('drop table t')", W, false),
    ]);
}

#[test]
fn writes_and_dangerous_dml() {
    check(&[
        ("INSERT INTO t VALUES (1)", W, false),
        (
            "INSERT INTO t VALUES (1) ON CONFLICT (id) DO UPDATE SET v = 1",
            W,
            false,
        ),
        ("UPDATE t SET v = 1 WHERE id = 1", W, false),
        ("UPDATE t SET v = 1", W, true),
        (
            "update t set v = (select max(v) from u where u.id = 1)",
            W,
            true,
        ),
        ("UPDATE t SET v = u.v FROM u", W, true),
        ("UPDATE t SET v = 1 WHERE CURRENT OF c", W, false),
        ("DELETE FROM t WHERE id = 1", W, false),
        ("DELETE FROM t", W, true),
        ("/* cleanup */ DELETE FROM t", W, true),
        ("DELETE FROM t USING u", W, true),
        ("DELETE FROM t WHERE id IN (SELECT id FROM u)", W, false),
        ("DELETE FROM t RETURNING (SELECT 1 WHERE true)", W, true),
        (
            "MERGE INTO t USING s ON t.id = s.id WHEN MATCHED THEN DELETE",
            W,
            false,
        ),
        ("TRUNCATE t", W, true),
        ("TRUNCATE TABLE a, b CASCADE", W, true),
        ("VACUUM ANALYZE t", W, false),
        ("ANALYZE t", W, false),
        ("REFRESH MATERIALIZED VIEW mv", W, false),
        ("LOCK TABLE t IN ACCESS EXCLUSIVE MODE", W, false),
        ("NOTIFY chan", W, false),
    ]);
}

#[test]
fn ddl() {
    check(&[
        (
            "CREATE TABLE t (id int REFERENCES u ON DELETE CASCADE)",
            D,
            false,
        ),
        ("CREATE TABLE t2 AS SELECT * FROM t", D, false),
        (
            "CREATE OR REPLACE FUNCTION f() RETURNS void AS $$ DELETE FROM t $$ LANGUAGE sql",
            D,
            false,
        ),
        (
            "CREATE RULE r AS ON DELETE TO t DO INSTEAD (DELETE FROM u)",
            D,
            true,
        ),
        ("DROP TABLE t", D, true),
        ("drop index if exists i", D, true),
        ("ALTER TABLE t ADD COLUMN c int", D, false),
        ("ALTER TABLE t DROP COLUMN c", D, true),
        ("ALTER TABLE t ALTER COLUMN c DROP NOT NULL", D, true),
        ("COMMENT ON TABLE t IS 'x'", D, false),
        ("GRANT SELECT ON t TO r", D, false),
        ("REVOKE ALL ON t FROM r", D, false),
    ]);
}

#[test]
fn with_queries() {
    check(&[
        ("WITH a AS (SELECT 1) SELECT * FROM a", R, false),
        (
            "WITH RECURSIVE a(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM a) SELECT * FROM a",
            R,
            false,
        ),
        (
            "WITH a AS MATERIALIZED (SELECT 1) SELECT * FROM a",
            R,
            false,
        ),
        (
            "WITH a AS (SELECT 1) DELETE FROM t WHERE id IN (SELECT * FROM a)",
            W,
            false,
        ),
        ("WITH a AS (SELECT 1) DELETE FROM t", W, true),
        ("WITH a AS (SELECT 1) UPDATE t SET v = 1", W, true),
        (
            "WITH a AS (SELECT 1) INSERT INTO t SELECT * FROM a",
            W,
            false,
        ),
        (
            "WITH d AS (DELETE FROM t WHERE id = 1 RETURNING *) SELECT * FROM d",
            W,
            false,
        ),
        (
            "WITH d AS (DELETE FROM t RETURNING *) SELECT * FROM d",
            W,
            true,
        ),
        (
            "WITH d AS NOT MATERIALIZED (UPDATE t SET v = 1 RETURNING *) SELECT 1",
            W,
            true,
        ),
        ("WITH a AS (SELECT 1)", U, false),
    ]);
}

#[test]
fn explain() {
    check(&[
        ("EXPLAIN SELECT 1", R, false),
        ("EXPLAIN DELETE FROM t", R, false),
        ("EXPLAIN VERBOSE UPDATE t SET v = 1", R, false),
        ("EXPLAIN (COSTS OFF, FORMAT JSON) DELETE FROM t", R, false),
        ("EXPLAIN (ANALYZE false) DELETE FROM t", R, false),
        ("EXPLAIN (ANALYZE off, BUFFERS) DELETE FROM t", R, false),
        ("EXPLAIN ANALYZE SELECT 1", R, false),
        ("EXPLAIN ANALYZE SELECT * FROM t FOR UPDATE", W, false),
        ("EXPLAIN ANALYZE DELETE FROM t", W, true),
        (
            "EXPLAIN ANALYSE VERBOSE DELETE FROM t WHERE id = 1",
            W,
            false,
        ),
        ("EXPLAIN (ANALYZE, BUFFERS) UPDATE t SET v = 1", W, true),
        ("EXPLAIN (ANALYZE true) INSERT INTO t VALUES (1)", W, false),
        (
            "EXPLAIN (ANALYZE) WITH d AS (DELETE FROM t RETURNING *) SELECT * FROM d",
            W,
            true,
        ),
        ("EXPLAIN ANALYZE", U, false),
        ("EXPLAIN (ANALYZE", U, false),
    ]);
}

#[test]
fn copy() {
    check(&[
        ("COPY t TO STDOUT", R, false),
        ("COPY t (a, b) TO STDOUT WITH (FORMAT csv)", R, false),
        ("COPY (SELECT * FROM t) TO STDOUT", R, false),
        ("COPY (DELETE FROM t RETURNING *) TO STDOUT", W, true),
        ("COPY t TO '/tmp/out.csv'", W, false),
        ("COPY t TO PROGRAM 'gzip > /tmp/x'", W, false),
        ("COPY t FROM STDIN", W, false),
        ("COPY t FROM PROGRAM 'cat x'", W, false),
        ("COPY t", U, false),
    ]);
}
