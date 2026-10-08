// SPDX-License-Identifier: Apache-2.0
//! `MySQL`/`MariaDB` safety classification: every statement that may modify
//! data or lift a guard must be a write (data-safety critical).

use super::check_in;
use crate::{Dialect, StatementKind};

use StatementKind::{Ddl as D, Read as R, TxControl as T, Unknown as U, Write as W};

const MY: Dialect = Dialect::MySql;

#[test]
fn reads() {
    check_in(
        MY,
        &[
            ("SELECT 1", R, false),
            (
                "# note\nSELECT `delete` FROM `update` WHERE note = 'drop table'",
                R,
                false,
            ),
            ("SELECT 'a\\' ; DELETE FROM t'", R, false),
            ("SHOW TABLES", R, false),
            ("SHOW CREATE TABLE t", R, false),
            ("DESCRIBE t", R, false),
            ("DESC t", R, false),
            ("EXPLAIN SELECT * FROM t", R, false),
            ("EXPLAIN FORMAT=JSON DELETE FROM t", R, false),
            ("EXPLAIN ANALYZE SELECT * FROM t", R, false),
            ("USE shop", R, false),
            ("HELP 'select'", R, false),
            ("TABLE t", R, false),
            ("VALUES ROW(1, 2)", R, false),
            ("CHECK TABLE t", R, false),
            ("SELECT REPLACE(name, 'a', 'b') FROM t", R, false),
            ("SELECT (REPLACE(name, 'a', 'b')) FROM t", R, false),
            ("WITH x AS (SELECT 1) SELECT * FROM x", R, false),
            ("SELECT 1 -- DELETE FROM t", R, false),
        ],
    );
}

#[test]
fn writes_and_reads_that_write() {
    check_in(
        MY,
        &[
            ("INSERT INTO t VALUES (1)", W, false),
            (
                "INSERT INTO t VALUES (1) ON DUPLICATE KEY UPDATE a = 2",
                W,
                false,
            ),
            ("REPLACE INTO t VALUES (1)", W, false),
            ("REPLACE t SET a = 1", W, false),
            ("UPDATE t SET a = 1 WHERE id = 2", W, false),
            ("DELETE FROM t WHERE id = 2", W, false),
            (
                "DELETE t1 FROM t1 JOIN t2 ON t1.id = t2.id WHERE t2.x",
                W,
                false,
            ),
            ("LOAD DATA INFILE 'x' INTO TABLE t", W, false),
            ("SELECT * INTO OUTFILE '/tmp/x' FROM t", W, false),
            ("SELECT a INTO @a FROM t", W, false),
            ("SELECT * FROM t FOR UPDATE", W, false),
            ("SELECT * FROM t LOCK IN SHARE MODE", W, false),
            ("SELECT GET_LOCK('x', 10)", W, false),
            ("SELECT NEXTVAL(s)", W, false),
            ("EXPLAIN ANALYZE UPDATE t SET a = 1 WHERE id = 1", W, false),
            ("ANALYZE TABLE t", W, false),
            ("ANALYZE DELETE FROM t WHERE id = 1", W, false),
            ("OPTIMIZE TABLE t", W, false),
            ("FLUSH PRIVILEGES", W, false),
            ("KILL 42", W, false),
            ("LOCK TABLES t WRITE", W, false),
            (
                "WITH x AS (SELECT 1) UPDATE t SET a = 1 WHERE id IN (SELECT * FROM x)",
                W,
                false,
            ),
        ],
    );
}

#[test]
fn dangerous_statements_need_confirmation() {
    check_in(
        MY,
        &[
            ("DELETE FROM t", W, true),
            ("DELETE FROM t LIMIT 10", W, true),
            ("UPDATE t SET a = 1", W, true),
            ("UPDATE t SET a = 1 ORDER BY id LIMIT 1", W, true),
            ("TRUNCATE TABLE t", W, true),
            ("TRUNCATE t", W, true),
            ("DROP TABLE t", D, true),
            ("DROP DATABASE shop", D, true),
            ("ALTER TABLE t DROP COLUMN a", D, true),
            ("SHUTDOWN", W, true),
            ("ANALYZE UPDATE t SET a = 1", W, true),
            ("EXPLAIN ANALYZE DELETE FROM t", W, true),
        ],
    );
}

#[test]
fn ddl() {
    check_in(
        MY,
        &[
            ("CREATE TABLE t (id INT)", D, false),
            ("ALTER TABLE t ADD COLUMN b INT", D, false),
            ("RENAME TABLE a TO b", D, false),
            ("GRANT SELECT ON *.* TO u", D, false),
            ("REVOKE ALL ON *.* FROM u", D, false),
            ("CREATE PROCEDURE p() BEGIN DELETE FROM t; END", D, false),
        ],
    );
}

#[test]
fn executable_comments_are_classified_as_code() {
    check_in(
        MY,
        &[
            ("/*!40101 SET NAMES utf8mb4 */", R, false),
            ("/*!40000 ALTER TABLE `t` DISABLE KEYS */", D, false),
            ("/*!50001 DROP VIEW IF EXISTS `v` */", D, true),
            ("SELECT 1 /*!, (DELETE FROM t) */", W, true),
            ("/*M!100100 DELETE FROM t */", W, true),
            ("/* DELETE FROM t */ SELECT 1", R, false),
            ("SELECT /*+ MAX_EXECUTION_TIME(1) */ 1", R, false),
        ],
    );
}

#[test]
fn transactions() {
    check_in(
        MY,
        &[
            ("BEGIN", T, false),
            ("BEGIN WORK", T, false),
            ("START TRANSACTION", T, false),
            ("START TRANSACTION READ ONLY", T, false),
            ("START TRANSACTION READ WRITE", U, false),
            ("START TRANSACTION WITH CONSISTENT SNAPSHOT", T, false),
            ("COMMIT", T, false),
            ("ROLLBACK", T, false),
            ("ROLLBACK TO SAVEPOINT s", T, false),
            ("SAVEPOINT s", T, false),
            ("RELEASE SAVEPOINT s", T, false),
            ("XA START 'x'", T, false),
            ("UNLOCK TABLES", T, false),
            ("BEGIN NOT ATOMIC DELETE FROM t; END", U, true),
        ],
    );
}

#[test]
fn session_settings() {
    check_in(
        MY,
        &[
            ("SET NAMES utf8mb4", R, false),
            ("SET CHARACTER SET utf8mb4", R, false),
            ("SET @a = 1", R, false),
            ("SET @a := (SELECT 1), @b = 2", R, false),
            ("SET autocommit = 0", R, false),
            ("SET SESSION time_zone = '+00:00'", R, false),
            ("SET @@session.sql_select_limit = 10", R, false),
            ("SET @@max_execution_time = 1000", R, false),
            ("set local wait_timeout = 10", R, false),
            ("SET TRANSACTION ISOLATION LEVEL READ COMMITTED", T, false),
            ("SET SESSION TRANSACTION READ ONLY", T, false),
            ("SET TRANSACTION READ WRITE", U, false),
            ("SET SESSION transaction_read_only = 0", U, false),
            ("SET @@session.transaction_read_only = OFF", U, false),
            ("SET GLOBAL max_connections = 10", U, false),
            ("SET @@global.time_zone = '+00:00'", U, false),
            ("SET PERSIST time_zone = '+00:00'", U, false),
            ("SET PASSWORD = 'x'", U, false),
            ("SET ROLE admin", U, false),
            ("SET sql_log_bin = 0", U, false),
            ("SET @a = 1, sql_log_bin = 0", U, false),
            (
                "SET STATEMENT max_statement_time = 1 FOR DELETE FROM t",
                U,
                false,
            ),
        ],
    );
}

#[test]
fn indirect_execution_is_unknown() {
    check_in(
        MY,
        &[
            ("CALL p()", U, false),
            ("DO SLEEP(1)", U, false),
            ("PREPARE s FROM 'DELETE FROM t'", U, false),
            ("EXECUTE s", U, false),
            ("HANDLER t OPEN", U, false),
            ("", U, false),
        ],
    );
}
