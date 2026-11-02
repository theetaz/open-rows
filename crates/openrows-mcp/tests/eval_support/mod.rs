// SPDX-License-Identifier: Apache-2.0
//! NL→SQL evaluation harness (docs/23 §8, M45-09). Each task's proposal comes
//! from a provider through the shared agent loop's `propose_sql` tool; the
//! proposal and the gold query then run through the same protected read-only
//! path as `run_read_query`, and the task is scored by result equivalence.
#![allow(
    dead_code,
    reason = "each evaluation binary uses the replay or the live provider"
)]
use openrows_ai::{RequestOptions, agent::*, sharing::*};
use openrows_driver_api::{ConnectRequest, ConnectTrace, DiscardSink, Driver};
use openrows_mcp::{McpError, ReadEngine, protected_read};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

mod agents;
use agents::ProposeTools;
pub use agents::{Replay, provider_config};
pub type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
type Rows = Vec<Vec<Option<String>>>;

/// The fixed evaluation set: fixture statements and tasks.
#[derive(Deserialize)]
pub struct EvalSet {
    pub fixture: Vec<String>,
    pub teardown: Vec<String>,
    pub tasks: Vec<Task>,
}
/// One question with its gold query and the provider proposal recorded for replay.
#[derive(Clone, Deserialize)]
pub struct Task {
    pub id: String,
    pub question: String,
    pub gold: String,
    pub recorded: String,
    /// Whether row order is part of the answer.
    pub ordered: bool,
    pub expect: Outcome,
}
/// How a proposal scored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    /// Same result set as the gold query.
    Equivalent,
    /// It ran, with a different result.
    Different,
    /// The read-only boundary refused it (write or unsafe construct).
    Refused,
    /// No proposal, or it failed to run.
    Failed,
}
/// One task's result, for the report.
#[derive(Debug, Serialize)]
pub struct Scored {
    pub id: String,
    pub outcome: Outcome,
}

pub fn load() -> Result<EvalSet> {
    Ok(serde_json::from_str(include_str!("../eval/nl_sql.json"))?)
}

/// A database under evaluation.
pub struct Target<'a> {
    pub driver: &'a dyn Driver,
    pub request: ConnectRequest,
    pub engine: ReadEngine,
    pub schema: &'a str,
    pub dialect: openrows_sql::Dialect,
}
impl Target<'_> {
    fn sql(&self, template: &str) -> String {
        template.replace("{schema}", self.schema)
    }
    async fn admin(&self, statements: &[String]) -> Result {
        let mut admin = self
            .driver
            .connect(&self.request, &mut ConnectTrace::default())
            .await?;
        for statement in statements {
            admin.execute(statement, &mut DiscardSink).await?;
        }
        admin.close().await?;
        Ok(())
    }
    async fn read(&self, sql: &str) -> std::result::Result<Rows, McpError> {
        let connection = self
            .driver
            .connect(&self.request, &mut ConnectTrace::default())
            .await
            .map_err(|_| McpError::Database)?;
        let bundle = protected_read(connection, self.engine, sql, false).await?;
        Ok(bundle
            .samples
            .into_iter()
            .next()
            .map(|sample| sample.rows)
            .unwrap_or_default())
    }
}

/// Create the fixture, score every task with `provider`, prove nothing changed, drop it.
pub async fn evaluate(
    target: &Target<'_>,
    set: &EvalSet,
    provider: &dyn Fn(&Task) -> Box<dyn AgentProvider>,
) -> Result<Vec<Scored>> {
    let _ = target.admin(&set.teardown).await;
    target.admin(&set.fixture).await?;
    let mut scored = Vec::new();
    for task in &set.tasks {
        let proposal = Box::pin(propose(provider(task).as_ref(), task, target)).await?;
        let outcome = score(target, task, proposal.as_deref()).await?;
        scored.push(Scored {
            id: task.id.clone(),
            outcome,
        });
    }
    let count = target
        .read(&target.sql("SELECT COUNT(*) FROM {schema}.eval_orders"))
        .await?;
    target.admin(&set.teardown).await?;
    if count != [[Some("10".to_owned())]] {
        return Err("an evaluated proposal changed the fixture".into());
    }
    Ok(scored)
}

async fn score(target: &Target<'_>, task: &Task, proposal: Option<&str>) -> Result<Outcome> {
    let gold = target.read(&target.sql(&task.gold)).await?;
    let Some(sql) = proposal else {
        return Ok(Outcome::Failed);
    };
    Ok(match target.read(sql).await {
        Err(McpError::Unsafe) => Outcome::Refused,
        Err(_) => Outcome::Failed,
        Ok(rows) => {
            if same(gold, rows, task.ordered) {
                Outcome::Equivalent
            } else {
                Outcome::Different
            }
        }
    })
}

fn same(mut gold: Rows, mut rows: Rows, ordered: bool) -> bool {
    if !ordered {
        gold.sort();
        rows.sort();
    }
    gold == rows
}

/// Ask `provider` for one proposal through the shared loop; the last one wins.
async fn propose(
    provider: &dyn AgentProvider,
    task: &Task,
    target: &Target<'_>,
) -> Result<Option<String>> {
    let tools = ProposeTools::new(target.dialect)?;
    let request = AgentRequest {
        model: std::env::var("OPENROWS_EVAL_MODEL").unwrap_or_else(|_| "replay".into()),
        context: tools.gate(ContextBundle {
            assistant: Some(AssistantContext {
                scope: None,
                history: vec![],
                question: task.question.clone(),
                error_code: None,
            }),
            schema: schema(target),
            ..ContextBundle::default()
        })?,
        limits: AgentLimits::default(),
        options: RequestOptions::default(),
    };
    let (send, mut receive) = mpsc::channel(64);
    let collect = async {
        let mut sql = None;
        while let Some(event) = receive.recv().await {
            if let AgentEvent::Proposal { sql: proposed } = event {
                sql = Some(proposed);
            }
        }
        sql
    };
    let (result, sql) = tokio::join!(run_agent(provider, &tools, &request, send), collect);
    result?;
    Ok(sql)
}

fn schema(target: &Target<'_>) -> Vec<SchemaObject> {
    [
        ("eval_customers", "id INTEGER PRIMARY KEY, name VARCHAR(64), country CHAR(2)"),
        (
            "eval_orders",
            "id INTEGER PRIMARY KEY, customer_id INTEGER REFERENCES eval_customers(id), total_cents INTEGER, status VARCHAR(16) ('paid', 'pending' or 'cancelled')",
        ),
    ]
    .into_iter()
    .map(|(table, columns)| SchemaObject {
        name: format!("{}.{table}", target.schema),
        ddl: format!("CREATE TABLE {}.{table} ({columns})", target.schema),
    })
    .collect()
}

/// Every task must score exactly as recorded.
pub fn assert_expected(set: &EvalSet, scored: &[Scored]) -> Result {
    for (task, result) in set.tasks.iter().zip(scored) {
        if task.expect != result.outcome {
            return Err(format!(
                "{}: expected {:?}, got {:?}",
                task.id, task.expect, result.outcome
            )
            .into());
        }
    }
    if scored.len() != set.tasks.len() {
        return Err("not every task was scored".into());
    }
    Ok(())
}
