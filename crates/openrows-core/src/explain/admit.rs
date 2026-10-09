// SPDX-License-Identifier: Apache-2.0
//! Whether a statement may be explained, under the connection's safety
//! switches. Pure, so every rule is unit-tested (docs/25 §0: data safety).
//!
//! A plan never runs the statement and is always allowed, even on read-only
//! connections. `ANALYZE` runs it: reads are allowed; anything that may
//! write is blocked on read-only connections, refused on engines that cannot
//! roll every write back (`MySQL`, `MariaDB`) and otherwise always needs the
//! user's confirmation, because side effects outside the transaction (such
//! as sequence values) are not undone.
use crate::safety::{SafetyPlan, SafetyPolicy, Verdict, verdict};
use openrows_explain::{AnalyzeWrites, Engine, Mode};
use openrows_sql::classify;

/// The gate's decision for one explain request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Admission {
    /// Explain without asking.
    Run,
    /// `ANALYZE` of a possible write: run only after the user confirmed.
    Confirm,
    /// Never on this connection (read-only).
    Block(String),
    /// Never on this engine (writes cannot be rolled back).
    Refuse(String),
}

/// Judge explaining `statement` with `mode` on `engine` under `policy`.
#[must_use]
pub fn admit(policy: SafetyPolicy, engine: Engine, mode: Mode, statement: &str) -> Admission {
    if !engine.executes(mode) {
        return Admission::Run;
    }
    let class = classify(statement, policy.dialect);
    if !class.is_write() && !class.dangerous {
        return Admission::Run;
    }
    if engine.analyze_writes() == AnalyzeWrites::Refuse {
        return Admission::Refuse(
            "Explain Analyze runs the statement, and this engine cannot roll every write \
             back. Use Explain to see the plan of a statement that may modify data."
                .to_owned(),
        );
    }
    let strict = SafetyPolicy {
        confirm_writes: true,
        ..policy
    };
    match verdict(strict, class) {
        Verdict::Block => Admission::Block(
            "This connection is read-only: Explain Analyze runs the statement, which may \
             modify the database. Use Explain to see its plan."
                .to_owned(),
        ),
        Verdict::Confirm | Verdict::Allow => Admission::Confirm,
    }
}

/// The token a confirmation of explaining `statement` on `scope` (the
/// session) must carry; any edit of the statement changes it.
#[must_use]
pub fn confirmation_token(policy: SafetyPolicy, scope: &str, statement: &str) -> String {
    let plan = SafetyPlan::check(policy, vec![statement.to_owned()]);
    plan.confirmation_token(&format!("explain-analyze:{scope}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use openrows_sql::Dialect;

    fn policy(read_only: bool, dialect: Dialect) -> SafetyPolicy {
        SafetyPolicy {
            read_only,
            confirm_writes: false,
            dialect,
        }
    }

    const PG: Engine = Engine::Postgres;

    #[test]
    fn plans_never_run_so_they_are_allowed_even_when_read_only() {
        let read_only = policy(true, Dialect::Postgres);
        assert_eq!(
            admit(read_only, PG, Mode::Plan, "DELETE FROM t"),
            Admission::Run
        );
        assert_eq!(
            admit(read_only, PG, Mode::Plan, "DROP TABLE t"),
            Admission::Run
        );
        let sqlite = policy(true, Dialect::Sqlite);
        assert_eq!(
            admit(sqlite, Engine::Sqlite, Mode::Analyze, "DELETE FROM t"),
            Admission::Run
        );
    }

    #[test]
    fn analysing_a_read_runs_without_asking() {
        for read_only in [false, true] {
            let policy = policy(read_only, Dialect::Postgres);
            assert_eq!(
                admit(policy, PG, Mode::Analyze, "SELECT * FROM t"),
                Admission::Run
            );
        }
        let mysql = policy(true, Dialect::MySql);
        assert_eq!(
            admit(mysql, Engine::MySql, Mode::Analyze, "SELECT 1"),
            Admission::Run
        );
    }

    #[test]
    fn analysing_a_write_always_asks_and_is_blocked_when_read_only() {
        let writable = policy(false, Dialect::Postgres);
        for sql in [
            "UPDATE t SET a = 1",
            "DELETE FROM t",
            "INSERT INTO t VALUES (1)",
            "frobnicate",
        ] {
            assert_eq!(
                admit(writable, PG, Mode::Analyze, sql),
                Admission::Confirm,
                "{sql}"
            );
        }
        let read_only = policy(true, Dialect::Postgres);
        assert!(matches!(
            admit(read_only, PG, Mode::Analyze, "UPDATE t SET a = 1"),
            Admission::Block(_)
        ));
    }

    #[test]
    fn mysql_and_mariadb_refuse_to_analyse_writes() {
        let policy = policy(false, Dialect::MySql);
        for engine in [Engine::MySql, Engine::MariaDb] {
            assert!(matches!(
                admit(policy, engine, Mode::Analyze, "DELETE FROM t"),
                Admission::Refuse(_)
            ));
            assert_eq!(
                admit(policy, engine, Mode::Plan, "DELETE FROM t"),
                Admission::Run
            );
        }
    }

    #[test]
    fn the_token_binds_the_statement_and_the_session() {
        let policy = policy(false, Dialect::Postgres);
        let token = confirmation_token(policy, "s1", "DELETE FROM t");
        assert_eq!(token, confirmation_token(policy, "s1", "DELETE FROM t"));
        assert_ne!(token, confirmation_token(policy, "s1", "DELETE FROM u"));
        assert_ne!(token, confirmation_token(policy, "s2", "DELETE FROM t"));
    }
}
