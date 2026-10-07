// SPDX-License-Identifier: Apache-2.0
//! Transaction, session, indirect-execution and fallback classification.

use super::{PG, check};
use crate::{StatementKind, classify};

use StatementKind::{Ddl as D, Read as R, TxControl as T, Unknown as U, Write as W};

#[test]
fn transactions_and_session() {
    check(&[
        ("BEGIN", T, false),
        ("BEGIN ISOLATION LEVEL SERIALIZABLE, READ ONLY", T, false),
        ("BEGIN READ WRITE", U, false),
        ("START TRANSACTION READ WRITE", U, false),
        ("START TRANSACTION", T, false),
        ("COMMIT", T, false),
        ("END", T, false),
        ("ROLLBACK TO SAVEPOINT s", T, false),
        ("ABORT", T, false),
        ("SAVEPOINT s", T, false),
        ("RELEASE SAVEPOINT s", T, false),
        ("PREPARE TRANSACTION 'x'", T, false),
        ("COMMIT PREPARED 'x'", T, false),
        ("SET TRANSACTION READ ONLY", T, false),
        ("SET TRANSACTION READ WRITE", U, false),
        (
            "SET SESSION CHARACTERISTICS AS TRANSACTION READ WRITE",
            U,
            false,
        ),
        ("SET search_path TO public", R, false),
        ("SET LOCAL statement_timeout = '5s'", R, false),
        ("SET SESSION TIME ZONE 'UTC'", R, false),
        ("set application_name = 'x'", R, false),
        ("SET default_transaction_read_only = off", U, false),
        ("SET transaction_read_only TO off", U, false),
        ("SET ROLE admin", U, false),
        ("SET SESSION AUTHORIZATION admin", U, false),
        ("SET search_path.x = 1", U, false),
        ("RESET search_path", R, false),
        ("RESET ALL", U, false),
        ("RESET ROLE", U, false),
    ]);
}

#[test]
fn unknown_and_indirect() {
    check(&[
        ("", U, false),
        ("-- just a comment", U, false),
        ("CALL do_things()", U, false),
        ("DO $$ BEGIN DELETE FROM t; END $$", U, false),
        ("EXECUTE stmt(1)", U, false),
        ("DISCARD ALL", U, false),
        ("LOAD 'plugin'", U, false),
        ("FROBNICATE everything", U, false),
        ("\"SELECT\" 1", U, false),
        ("PREPARE q AS SELECT 1", R, false),
        ("PREPARE q (int) AS DELETE FROM t", W, true),
        ("PREPARE q", U, false),
        ("DECLARE c CURSOR FOR SELECT 1", R, false),
        (
            "DECLARE c NO SCROLL CURSOR WITH HOLD FOR SELECT * FROM t FOR UPDATE",
            W,
            false,
        ),
        ("DECLARE c", U, false),
    ]);
}

#[test]
fn embedded_semicolons_never_read() {
    check(&[
        ("SELECT (1; DELETE FROM t", W, true),
        ("SELECT (1; SELECT 2", U, false),
        ("EXPLAIN SELECT (1; DROP TABLE t", U, false),
        (
            "CREATE RULE r AS ON INSERT TO t DO ALSO (DELETE FROM a WHERE x; DELETE FROM b)",
            D,
            true,
        ),
    ]);
}

#[test]
fn write_predicate() {
    assert!(!StatementKind::Read.is_write());
    assert!(!StatementKind::TxControl.is_write());
    assert!(StatementKind::Write.is_write());
    assert!(StatementKind::Ddl.is_write());
    assert!(StatementKind::Unknown.is_write());
    assert!(classify("FOO", PG).is_write());
}
