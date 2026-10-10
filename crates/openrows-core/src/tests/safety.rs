// SPDX-License-Identifier: Apache-2.0
//! Safety gate: read-only blocking, confirm-writes and dangerous statements.
//! Data-safety critical (docs/25 §0): every rule and its combinations.
use crate::safety::{Gate, SafetyPlan, SafetyPolicy, Verdict, verdict};
use openrows_sql::{Classification, Dialect, StatementKind, classify};

const OPEN: SafetyPolicy = SafetyPolicy {
    mode: openrows_types::safety::Safety::Unrestricted,
    dialect: Dialect::Postgres,
};
const CONFIRM: SafetyPolicy = SafetyPolicy {
    mode: openrows_types::safety::Safety::ConfirmWrites,
    dialect: Dialect::Postgres,
};
const READ_ONLY: SafetyPolicy = SafetyPolicy {
    mode: openrows_types::safety::Safety::ReadOnly,
    dialect: Dialect::Postgres,
};
const LOCKED: SafetyPolicy = SafetyPolicy {
    mode: openrows_types::safety::Safety::ReadOnly,
    dialect: Dialect::Postgres,
};

fn judge(policy: SafetyPolicy, sql: &str) -> Verdict {
    verdict(policy, classify(sql, Dialect::Postgres))
}

fn plan(policy: SafetyPolicy, statements: &[&str]) -> SafetyPlan {
    SafetyPlan::check(
        policy,
        statements.iter().map(|sql| (*sql).to_owned()).collect(),
    )
}

/// Statements that only read (or control transactions) on `PostgreSQL`.
const READS: &[&str] = &[
    "SELECT 1",
    "select * from orders where id = 3",
    "WITH t AS (SELECT 1) SELECT * FROM t",
    "SHOW search_path",
    "EXPLAIN SELECT * FROM orders",
    "VALUES (1), (2)",
    "TABLE orders",
    "BEGIN",
    "START TRANSACTION READ ONLY",
    "COMMIT",
    "ROLLBACK",
    "SAVEPOINT a",
    "SET search_path TO app, public",
    "SET statement_timeout = '5s'",
    "-- comment first\nSELECT 2",
];

/// Writes, DDL and unrecognised statements that are not dangerous.
const MODIFYING: &[&str] = &[
    "INSERT INTO orders (id) VALUES (1)",
    "UPDATE orders SET total = 1 WHERE id = 1",
    "DELETE FROM orders WHERE id = 1",
    "CREATE TABLE t (id int)",
    "CREATE INDEX ON orders (id)",
    "GRANT SELECT ON orders TO bob",
    "COMMENT ON TABLE orders IS 'x'",
    "COPY orders FROM '/tmp/x.csv'",
    "VACUUM orders",
    "SELECT nextval('orders_id_seq')",
    "SELECT * INTO copy FROM orders",
    "SELECT * FROM orders FOR UPDATE",
    "WITH gone AS (DELETE FROM orders WHERE id = 1 RETURNING *) SELECT * FROM gone",
    "EXPLAIN ANALYZE INSERT INTO orders VALUES (1)",
    "SET ROLE admin",
    "SET default_transaction_read_only = off",
    "RESET ALL",
    "BEGIN READ WRITE",
    "DO $$ BEGIN PERFORM 1; END $$",
    "CALL refresh_all()",
    "EXECUTE prepared_stmt",
    "frobnicate the database",
];

/// Dangerous statements: unbounded DML, DROP, TRUNCATE, ALTER … DROP.
const DANGEROUS: &[&str] = &[
    "DELETE FROM orders",
    "UPDATE orders SET total = 0",
    "DROP TABLE orders",
    "DROP DATABASE shop",
    "TRUNCATE orders",
    "ALTER TABLE orders DROP COLUMN notes",
    "WITH gone AS (DELETE FROM orders RETURNING *) SELECT count(*) FROM gone",
];

#[test]
fn fixtures_classify_as_described() {
    for sql in READS {
        let class = classify(sql, Dialect::Postgres);
        assert!(!class.is_write() && !class.dangerous, "{sql}: {class:?}");
    }
    for sql in MODIFYING {
        let class = classify(sql, Dialect::Postgres);
        assert!(class.is_write() && !class.dangerous, "{sql}: {class:?}");
    }
    for sql in DANGEROUS {
        assert!(classify(sql, Dialect::Postgres).dangerous, "{sql}");
    }
}

#[test]
fn reads_run_under_every_policy() {
    for policy in [OPEN, CONFIRM, READ_ONLY, LOCKED] {
        for sql in READS {
            assert_eq!(judge(policy, sql), Verdict::Allow, "{sql} {policy:?}");
        }
    }
}

#[test]
fn read_only_blocks_writes_ddl_unknown_and_dangerous() {
    for policy in [READ_ONLY, LOCKED] {
        for sql in MODIFYING.iter().chain(DANGEROUS) {
            assert_eq!(judge(policy, sql), Verdict::Block, "{sql} {policy:?}");
        }
    }
}

#[test]
fn confirm_writes_asks_for_every_modifying_statement() {
    for sql in MODIFYING.iter().chain(DANGEROUS) {
        assert_eq!(judge(CONFIRM, sql), Verdict::Confirm, "{sql}");
    }
}

#[test]
fn dangerous_statements_always_need_confirmation() {
    for sql in DANGEROUS {
        assert_eq!(judge(OPEN, sql), Verdict::Confirm, "{sql}");
    }
}

#[test]
fn bounded_writes_run_without_confirm_writes() {
    for sql in MODIFYING {
        assert_eq!(judge(OPEN, sql), Verdict::Allow, "{sql}");
    }
}

#[test]
fn verdict_matrix_over_every_kind() {
    let kinds = [
        (StatementKind::Read, false),
        (StatementKind::TxControl, false),
        (StatementKind::Write, true),
        (StatementKind::Ddl, true),
        (StatementKind::Unknown, true),
    ];
    for (kind, modifies) in kinds {
        for dangerous in [false, true] {
            let class = Classification { kind, dangerous };
            let risky = modifies || dangerous;
            let expect = |policy: SafetyPolicy| match () {
                () if policy.mode.read_only() && risky => Verdict::Block,
                () if dangerous || (policy.mode.confirm_writes() && risky) => Verdict::Confirm,
                () => Verdict::Allow,
            };
            for policy in [OPEN, CONFIRM, READ_ONLY, LOCKED] {
                assert_eq!(
                    verdict(policy, class),
                    expect(policy),
                    "{class:?} {policy:?}"
                );
            }
        }
    }
}

#[test]
fn one_blocked_statement_refuses_the_whole_run() {
    let run = plan(READ_ONLY, &["SELECT 1", "DELETE FROM orders WHERE id = 1"]);
    assert_eq!(run.gate(), Gate::Block);
    assert_eq!(run.statements[0].verdict, Verdict::Allow);
    assert_eq!(run.statements[1].verdict, Verdict::Block);
    let token = run.confirmation_token("s");
    assert!(!run.allows("s", Some(&token)));
    assert!(!run.allows("s", None));
}

#[test]
fn confirmation_is_bound_to_the_exact_statements_and_scope() {
    let run = plan(OPEN, &["SELECT 1", "DROP TABLE orders"]);
    assert_eq!(run.gate(), Gate::Confirm);
    let token = run.confirmation_token("session-a");
    assert!(!run.allows("session-a", None));
    assert!(!run.allows("session-a", Some("0000000000000000")));
    assert!(!run.allows("session-b", Some(&token)));
    assert!(run.allows("session-a", Some(&token)));
    let edited = plan(OPEN, &["SELECT 1", "DROP TABLE customers"]);
    assert!(!edited.allows("session-a", Some(&token)));
    let extended = plan(OPEN, &["SELECT 1", "DROP TABLE orders", "TRUNCATE x"]);
    assert!(!extended.allows("session-a", Some(&token)));
}

#[test]
fn allowed_runs_need_no_token_and_empty_runs_are_allowed() {
    let run = plan(CONFIRM, &["SELECT 1", "BEGIN", "COMMIT"]);
    assert_eq!(run.gate(), Gate::Run);
    assert!(run.allows("s", None));
    assert_eq!(plan(LOCKED, &[]).gate(), Gate::Run);
}

#[test]
fn tokens_are_stable_for_identical_runs() {
    let first = plan(CONFIRM, &["INSERT INTO t VALUES (1)"]);
    let second = plan(CONFIRM, &["INSERT INTO t VALUES (1)"]);
    assert_eq!(
        first.confirmation_token("s"),
        second.confirmation_token("s")
    );
    assert_eq!(first.confirmation_token("s").len(), 16);
}

/// The same gate holds for `MySQL` and `SQLite` statements, classified with
/// their own dialect (D-12).
#[test]
fn mysql_and_sqlite_writes_are_blocked_on_read_only_connections() {
    let cases = [
        (Dialect::MySql, "REPLACE INTO t VALUES (1)"),
        (Dialect::MySql, "/*!40000 ALTER TABLE `t` DISABLE KEYS */"),
        (Dialect::MySql, "SET GLOBAL read_only = 0"),
        (Dialect::MySql, "SELECT * FROM t LOCK IN SHARE MODE"),
        (Dialect::Sqlite, "INSERT OR REPLACE INTO t VALUES (1)"),
        (Dialect::Sqlite, "PRAGMA journal_mode = DELETE"),
        (Dialect::Sqlite, "ATTACH 'x.db' AS x"),
    ];
    for (dialect, sql) in cases {
        let policy = SafetyPolicy {
            dialect,
            ..READ_ONLY
        };
        let checked = SafetyPlan::check(policy, vec![sql.to_owned()]);
        assert_eq!(checked.gate(), Gate::Block, "{dialect:?}: {sql}");
    }
    let reads = [
        (Dialect::MySql, "SELECT `delete` FROM t # DROP TABLE t"),
        (Dialect::MySql, "SHOW CREATE TABLE t"),
        (Dialect::Sqlite, "PRAGMA table_info([t])"),
    ];
    for (dialect, sql) in reads {
        let policy = SafetyPolicy {
            dialect,
            ..READ_ONLY
        };
        let checked = SafetyPlan::check(policy, vec![sql.to_owned()]);
        assert_eq!(checked.gate(), Gate::Run, "{dialect:?}: {sql}");
    }
}

#[test]
fn mysql_and_sqlite_dangerous_statements_always_confirm() {
    let cases = [
        (Dialect::MySql, "DELETE FROM t"),
        (Dialect::MySql, "TRUNCATE t"),
        (Dialect::MySql, "DROP DATABASE shop"),
        (Dialect::Sqlite, "UPDATE t SET a = 1"),
        (Dialect::Sqlite, "DROP TABLE t"),
    ];
    for (dialect, sql) in cases {
        let policy = SafetyPolicy { dialect, ..OPEN };
        let checked = SafetyPlan::check(policy, vec![sql.to_owned()]);
        assert_eq!(checked.gate(), Gate::Confirm, "{dialect:?}: {sql}");
    }
}
