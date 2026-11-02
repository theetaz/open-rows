// SPDX-License-Identifier: Apache-2.0
//! NL→SQL evaluation over per-dialect fixtures, scored by result equivalence
//! (docs/23 §8, M45-09). CI replays recorded proposals with no network; set
//! `OPENROWS_EVAL_LIVE=1` to ask a real provider instead (see docs/23 §17).
mod eval_support;
#[path = "support/read.rs"]
mod fixture;
use eval_support::{Replay, Result, Target, assert_expected, evaluate, load};
use openrows_ai::{AiControl, HttpProvider, NetworkOptions, ProviderKind, agent::AgentProvider};
use openrows_mcp::ReadEngine;
use openrows_sql::Dialect;

async fn replayed(target: &Target<'_>) -> Result {
    let set = load()?;
    let schema = target.schema.to_owned();
    let scored = evaluate(target, &set, &|task| Box::new(Replay::new(task, &schema))).await?;
    assert_expected(&set, &scored)
}

#[tokio::test]
async fn sqlite_replayed_proposals_score_by_result_equivalence() -> Result {
    let directory = tempfile::tempdir()?;
    let target = Target {
        driver: &openrows_driver_sqlite::SqliteDriver,
        request: fixture::file_request(&directory.path().join("eval.sqlite")),
        engine: ReadEngine::Sqlite,
        schema: "main",
        dialect: Dialect::Sqlite,
    };
    replayed(&target).await
}

#[tokio::test]
#[ignore = "requires just test-pg's single native PostgreSQL fixture"]
async fn postgres_replayed_proposals_score_by_result_equivalence() -> Result {
    let target = Target {
        driver: &openrows_driver_postgres::PgDriver,
        request: fixture::server_request(
            "OPENROWS_TEST_PG_PORT",
            "OPENROWS_TEST_PG_PASSWORD",
            "postgres",
            "postgres",
        )?,
        engine: ReadEngine::Postgres,
        schema: "public",
        dialect: Dialect::Postgres,
    };
    replayed(&target).await
}

#[tokio::test]
#[ignore = "requires just test-mysql's single native MySQL fixture"]
async fn mysql_replayed_proposals_score_by_result_equivalence() -> Result {
    let target = Target {
        driver: &openrows_driver_mysql::MySqlDriver(openrows_driver_mysql::Flavour::MySql),
        request: fixture::server_request(
            "OPENROWS_TEST_MYSQL_PORT",
            "OPENROWS_TEST_MYSQL_PASSWORD",
            "root",
            "openrows_test",
        )?,
        engine: ReadEngine::Mysql,
        schema: "openrows_test",
        dialect: Dialect::MySql,
    };
    replayed(&target).await
}

/// Opt-in live run against a real provider on the `SQLite` fixture. Needs
/// `OPENROWS_EVAL_LIVE=1`, `OPENROWS_EVAL_PROVIDER` (for example `anthropic`),
/// `OPENROWS_EVAL_MODEL`, optionally `OPENROWS_EVAL_BASE_URL` and
/// `OPENROWS_EVAL_API_KEY`; writes the per-task scores to
/// `OPENROWS_EVAL_REPORT` when set. Live scores are recorded, not asserted.
#[tokio::test]
#[ignore = "opt-in: sends the fixture schema and questions to a real provider"]
async fn live_provider_nl_sql_evaluation() -> Result {
    if std::env::var("OPENROWS_EVAL_LIVE").as_deref() != Ok("1") {
        return Ok(());
    }
    let kind: ProviderKind = serde_json::from_value(serde_json::Value::String(std::env::var(
        "OPENROWS_EVAL_PROVIDER",
    )?))?;
    let base =
        std::env::var("OPENROWS_EVAL_BASE_URL").unwrap_or_else(|_| kind.default_url().into());
    let key = std::env::var("OPENROWS_EVAL_API_KEY").ok();
    let config = eval_support::provider_config(kind, &base, key.is_some());
    let control = AiControl::default();
    control.set_enabled(true);
    let directory = tempfile::tempdir()?;
    let target = Target {
        driver: &openrows_driver_sqlite::SqliteDriver,
        request: fixture::file_request(&directory.path().join("eval.sqlite")),
        engine: ReadEngine::Sqlite,
        schema: "main",
        dialect: Dialect::Sqlite,
    };
    let live = |_: &eval_support::Task| -> Box<dyn AgentProvider> {
        let secret = key.clone().map(openrows_secrets::SecretString::new);
        match HttpProvider::new(&config, secret, control.clone(), NetworkOptions::default()) {
            Ok(provider) => Box::new(provider.with_tool_calling(true)),
            Err(_) => Box::new(Replay::new(&unavailable(), "main")),
        }
    };
    let scored = evaluate(&target, &load()?, &live).await?;
    if let Ok(path) = std::env::var("OPENROWS_EVAL_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&scored)?)?;
    }
    Ok(())
}

/// A provider that cannot be built proposes nothing that runs.
fn unavailable() -> eval_support::Task {
    eval_support::Task {
        id: String::new(),
        question: String::new(),
        gold: String::new(),
        recorded: "provider unavailable".into(),
        ordered: false,
        expect: eval_support::Outcome::Failed,
    }
}
