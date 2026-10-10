// SPDX-License-Identifier: Apache-2.0
//! The statements that explain one statement on each engine.
//!
//! `PostgreSQL` gets `EXPLAIN (FORMAT JSON, …)`; `ANALYZE` runs inside a
//! transaction (or a savepoint of the user's open transaction) that is always
//! rolled back. `MySQL` uses `EXPLAIN FORMAT=JSON` and `EXPLAIN ANALYZE`
//! (tree text), `MariaDB` `EXPLAIN`/`ANALYZE FORMAT=JSON`; neither can roll
//! back every engine's writes, so they analyse reads only. `SQLite` has only
//! `EXPLAIN QUERY PLAN`, which never runs the statement.
use crate::model::{Engine, Format};

/// Estimated plan only, or run the statement and measure it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    /// `EXPLAIN`: the planner's estimates; the statement does not run.
    Plan,
    /// `EXPLAIN ANALYZE`: the statement runs and actuals are reported.
    Analyze,
}

/// How an analysed statement's effects are undone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Wrap {
    /// Nothing to undo (plans, reads on engines that refuse writes).
    None,
    /// `BEGIN` … `ROLLBACK`: no transaction was open.
    Transaction,
    /// `SAVEPOINT` … `ROLLBACK TO SAVEPOINT`: keeps the user's open transaction.
    Savepoint,
}

/// What `ANALYZE` of a statement that may write does on an engine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnalyzeWrites {
    /// The statement runs inside a transaction that is rolled back.
    RollBack,
    /// The engine cannot undo every write: analysing writes is refused.
    Refuse,
    /// The engine never runs the statement (`SQLite` query plans).
    NotRun,
}

/// Name of the savepoint used inside a user's open transaction.
pub const SAVEPOINT: &str = "openrows_explain";

/// The statements to run, in order: `before`, then `explain` (whose rows
/// are the plan), then `after`, which must run even when `explain` failed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Command {
    /// Set-up, e.g. `BEGIN`.
    pub before: Vec<String>,
    /// The explaining statement.
    pub explain: String,
    /// Clean-up, e.g. `ROLLBACK`.
    pub after: Vec<String>,
    /// How the plan rows are encoded.
    pub format: Format,
    /// The plan will carry actual rows and times.
    pub analyzed: bool,
}

impl Engine {
    /// What analysing a statement that may write does on this engine.
    #[must_use]
    pub fn analyze_writes(self) -> AnalyzeWrites {
        match self {
            Self::Postgres | Self::SqlServer => AnalyzeWrites::RollBack,
            Self::MySql | Self::MariaDb => AnalyzeWrites::Refuse,
            Self::Sqlite => AnalyzeWrites::NotRun,
        }
    }

    /// The engine actually executes the statement for `mode`.
    #[must_use]
    pub fn executes(self, mode: Mode) -> bool {
        mode == Mode::Analyze && self != Self::Sqlite
    }
}

/// The statements explaining `statement` on `engine`. `wrap` applies to
/// `PostgreSQL` `ANALYZE` only: a savepoint when the session already has a
/// transaction open, else a transaction.
#[must_use]
pub fn command(engine: Engine, mode: Mode, statement: &str, wrap: Wrap) -> Command {
    let statement = statement.trim().trim_end_matches(';').trim_end();
    let plain = |explain: String, format: Format, analyzed: bool| Command {
        before: Vec::new(),
        explain,
        after: Vec::new(),
        format,
        analyzed,
    };
    match (engine, mode) {
        (Engine::Postgres, Mode::Plan) => plain(
            format!("EXPLAIN (FORMAT JSON, VERBOSE, SETTINGS) {statement}"),
            Format::PostgresJson,
            false,
        ),
        (Engine::Postgres, Mode::Analyze) => postgres_analyze(statement, wrap),
        (Engine::SqlServer, mode) => sqlserver(statement, mode, wrap),
        (Engine::MySql | Engine::MariaDb, Mode::Plan) => plain(
            format!("EXPLAIN FORMAT=JSON {statement}"),
            Format::MySqlJson,
            false,
        ),
        (Engine::MySql, Mode::Analyze) => plain(
            format!("EXPLAIN ANALYZE {statement}"),
            Format::MySqlTree,
            true,
        ),
        (Engine::MariaDb, Mode::Analyze) => plain(
            format!("ANALYZE FORMAT=JSON {statement}"),
            Format::MySqlJson,
            true,
        ),
        (Engine::Sqlite, _) => plain(
            format!("EXPLAIN QUERY PLAN {statement}"),
            Format::SqliteRows,
            false,
        ),
    }
}

fn postgres_analyze(statement: &str, wrap: Wrap) -> Command {
    let (before, after) = match wrap {
        Wrap::None => (Vec::new(), Vec::new()),
        Wrap::Transaction => (vec!["BEGIN".to_owned()], vec!["ROLLBACK".to_owned()]),
        Wrap::Savepoint => (
            vec![format!("SAVEPOINT {SAVEPOINT}")],
            vec![
                format!("ROLLBACK TO SAVEPOINT {SAVEPOINT}"),
                format!("RELEASE SAVEPOINT {SAVEPOINT}"),
            ],
        ),
    };
    Command {
        before,
        explain: format!("EXPLAIN (FORMAT JSON, ANALYZE, BUFFERS, VERBOSE, SETTINGS) {statement}"),
        after,
        format: Format::PostgresJson,
        analyzed: true,
    }
}

fn sqlserver(statement: &str, mode: Mode, wrap: Wrap) -> Command {
    let option = if mode == Mode::Plan {
        "SHOWPLAN_XML"
    } else {
        "STATISTICS XML"
    };
    let (mut before, mut after) = match wrap {
        Wrap::None => (Vec::new(), Vec::new()),
        Wrap::Transaction => (
            vec!["BEGIN TRAN".to_owned()],
            vec!["IF @@TRANCOUNT > 0 ROLLBACK TRAN".to_owned()],
        ),
        Wrap::Savepoint => (
            vec![format!("SAVE TRAN {SAVEPOINT}")],
            vec![format!(
                "IF XACT_STATE() = 1 ROLLBACK TRAN {SAVEPOINT} ELSE IF XACT_STATE() = -1 THROW 50000, 'Explain transaction cannot roll back to its savepoint; roll back the transaction', 1"
            )],
        ),
    };
    // SHOWPLAN must be the only statement in its batch; turn it off before rollback.
    before.push(format!("SET {option} ON"));
    after.insert(0, format!("SET {option} OFF"));
    Command {
        before,
        explain: statement.to_owned(),
        after,
        format: Format::SqlServerXml,
        analyzed: mode == Mode::Analyze,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn postgres_plans_in_json_and_analyses_inside_a_rolled_back_transaction() {
        let plan = command(Engine::Postgres, Mode::Plan, "SELECT 1;", Wrap::None);
        assert_eq!(
            plan.explain,
            "EXPLAIN (FORMAT JSON, VERBOSE, SETTINGS) SELECT 1"
        );
        assert!(plan.before.is_empty() && plan.after.is_empty() && !plan.analyzed);
        let analyze = command(
            Engine::Postgres,
            Mode::Analyze,
            "DELETE FROM t",
            Wrap::Transaction,
        );
        assert_eq!(analyze.before, ["BEGIN"]);
        assert_eq!(analyze.after, ["ROLLBACK"]);
        assert!(
            analyze
                .explain
                .starts_with("EXPLAIN (FORMAT JSON, ANALYZE, BUFFERS")
        );
        assert!(analyze.analyzed);
    }

    #[test]
    fn an_open_transaction_is_kept_with_a_savepoint() {
        let analyze = command(
            Engine::Postgres,
            Mode::Analyze,
            "UPDATE t SET a = 1",
            Wrap::Savepoint,
        );
        assert_eq!(analyze.before, ["SAVEPOINT openrows_explain"]);
        assert_eq!(
            analyze.after,
            [
                "ROLLBACK TO SAVEPOINT openrows_explain",
                "RELEASE SAVEPOINT openrows_explain"
            ]
        );
    }

    #[test]
    fn mysql_mariadb_and_sqlite_use_their_own_syntax() {
        let cases = [
            (
                Engine::MySql,
                Mode::Plan,
                "EXPLAIN FORMAT=JSON SELECT 1",
                Format::MySqlJson,
            ),
            (
                Engine::MySql,
                Mode::Analyze,
                "EXPLAIN ANALYZE SELECT 1",
                Format::MySqlTree,
            ),
            (
                Engine::MariaDb,
                Mode::Plan,
                "EXPLAIN FORMAT=JSON SELECT 1",
                Format::MySqlJson,
            ),
            (
                Engine::MariaDb,
                Mode::Analyze,
                "ANALYZE FORMAT=JSON SELECT 1",
                Format::MySqlJson,
            ),
            (
                Engine::Sqlite,
                Mode::Analyze,
                "EXPLAIN QUERY PLAN SELECT 1",
                Format::SqliteRows,
            ),
        ];
        for (engine, mode, sql, format) in cases {
            let command = command(engine, mode, " SELECT 1 ; ", Wrap::Transaction);
            assert_eq!((command.explain.as_str(), command.format), (sql, format));
            assert!(command.before.is_empty() && command.after.is_empty());
        }
    }

    #[test]
    fn only_postgres_rolls_analysed_writes_back() {
        assert_eq!(Engine::Postgres.analyze_writes(), AnalyzeWrites::RollBack);
        assert_eq!(Engine::MySql.analyze_writes(), AnalyzeWrites::Refuse);
        assert_eq!(Engine::MariaDb.analyze_writes(), AnalyzeWrites::Refuse);
        assert_eq!(Engine::Sqlite.analyze_writes(), AnalyzeWrites::NotRun);
        assert!(!Engine::Sqlite.executes(Mode::Analyze));
        assert!(!Engine::Postgres.executes(Mode::Plan));
    }
    #[test]
    fn sqlserver_options_have_their_own_batches_and_reset_before_rollback() {
        let plan = command(Engine::SqlServer, Mode::Plan, "SELECT 1", Wrap::None);
        assert_eq!(plan.before, ["SET SHOWPLAN_XML ON"]);
        assert_eq!(plan.after, ["SET SHOWPLAN_XML OFF"]);
        assert_eq!(plan.explain, "SELECT 1");
        let actual = command(
            Engine::SqlServer,
            Mode::Analyze,
            "UPDATE t SET x=1",
            Wrap::Transaction,
        );
        assert_eq!(actual.before, ["BEGIN TRAN", "SET STATISTICS XML ON"]);
        assert_eq!(
            actual.after,
            ["SET STATISTICS XML OFF", "IF @@TRANCOUNT > 0 ROLLBACK TRAN"]
        );
        let saved = command(
            Engine::SqlServer,
            Mode::Analyze,
            "SELECT 1",
            Wrap::Savepoint,
        );
        assert_eq!(saved.before[0], "SAVE TRAN openrows_explain");
        assert!(saved.after[1].contains("ROLLBACK TRAN openrows_explain"));
    }
}
