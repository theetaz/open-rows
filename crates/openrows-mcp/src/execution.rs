// SPDX-License-Identifier: Apache-2.0
//! Protected disposable sessions, shared by MCP and future assistant callers.
use crate::{McpError, rows::Collector};
use openrows_ai::sharing::{ContextBundle, Plan, PlanOperation, Sample};
use openrows_driver_api::{CancelHandle, Connection, DiscardSink};
use openrows_explain::{Engine, Mode, Wrap, analyse, command, parse};
use openrows_sql::{Dialect, ai_read_source, validate_ai_read};
use std::{sync::Arc, time::Duration};

/// All engines have an explicit AI execution decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadEngine {
    /// `PostgreSQL` read-only transaction and native statement timeout.
    Postgres,
    /// `MySQL` read-only transaction and maximum SELECT execution time.
    Mysql,
    /// `MariaDB` read-only transaction and maximum statement time.
    Mariadb,
    /// `SQLite` query-only session.
    Sqlite,
    /// libSQL file uses the same local `SQLite` query-only engine.
    LibsqlFile,
    /// Remote libSQL protection has not been validated; refuse reads.
    Libsql,
    /// SQL Server lacks this execution protection; refuse reads.
    Sqlserver,
    /// `ClickHouse` protection has not been validated; refuse reads.
    Clickhouse,
    /// Hosted `DuckDB`'s extra sessions are unavailable; refuse reads.
    Duckdb,
}
impl ReadEngine {
    fn setup(self) -> Result<&'static [&'static str], McpError> {
        match self {
            Self::Postgres => Ok(&[
                "BEGIN READ ONLY",
                "SET LOCAL statement_timeout = 5000",
                "SET LOCAL lock_timeout = 1000",
                "SET LOCAL search_path = pg_catalog",
                "SET LOCAL standard_conforming_strings = on",
                "SET LOCAL row_security = off",
            ]),
            Self::Mysql => Ok(&[
                "SET SESSION MAX_EXECUTION_TIME=5000",
                "START TRANSACTION READ ONLY",
            ]),
            Self::Mariadb => Ok(&[
                "SET SESSION max_statement_time=5",
                "START TRANSACTION READ ONLY",
            ]),
            Self::Sqlite | Self::LibsqlFile => Ok(&["PRAGMA query_only=ON", "BEGIN"]),
            Self::Libsql | Self::Sqlserver | Self::Clickhouse | Self::Duckdb => {
                Err(McpError::Unsafe)
            }
        }
    }
    fn explain(self) -> Result<(Engine, Dialect), McpError> {
        match self {
            Self::Postgres => Ok((Engine::Postgres, Dialect::Postgres)),
            Self::Mysql => Ok((Engine::MySql, Dialect::MySql)),
            Self::Mariadb => Ok((Engine::MariaDb, Dialect::MySql)),
            Self::Sqlite | Self::LibsqlFile => Ok((Engine::Sqlite, Dialect::Sqlite)),
            Self::Libsql | Self::Sqlserver | Self::Clickhouse | Self::Duckdb => {
                Err(McpError::Unsafe)
            }
        }
    }
}
/// Execute on a fresh disposable connection with engine protection and hard collection bounds.
/// Dropping this future interrupts the driver and drops the connection; no reconnect/replay.
/// # Errors
/// Unsafe SQL/engine, deadline, database or cleanup failure, with no vendor text.
pub async fn protected_read(
    connection: Box<dyn Connection>,
    engine: ReadEngine,
    sql: &str,
    plan: bool,
) -> Result<ContextBundle, McpError> {
    let (_, dialect) = engine.explain()?;
    let sql = validate_ai_read(sql, dialect).map_err(|_| McpError::Unsafe)?;
    tokio::time::timeout(
        Duration::from_secs(5),
        on_connection(connection, engine, sql, plan),
    )
    .await
    .map_err(|_| McpError::Limit)?
}
async fn on_connection(
    mut connection: Box<dyn Connection>,
    engine: ReadEngine,
    sql: &str,
    plan: bool,
) -> Result<ContextBundle, McpError> {
    let (explain_engine, dialect) = engine.explain()?;
    let source = ai_read_source(sql, dialect).map_err(|_| McpError::Unsafe)?;
    let mut guard = Interrupt(connection.cancel_handle());
    let result = collect(
        connection.as_mut(),
        (engine, explain_engine),
        (sql, source),
        plan,
    )
    .await;
    connection
        .execute("ROLLBACK", &mut DiscardSink)
        .await
        .map_err(|_| McpError::Database)?;
    guard.0 = None;
    connection.close().await.map_err(|_| McpError::Database)?;
    result
}

async fn collect(
    connection: &mut dyn Connection,
    (engine, explain_engine): (ReadEngine, Engine),
    (sql, source): (&str, Option<(String, String)>),
    plan: bool,
) -> Result<ContextBundle, McpError> {
    for setup in engine.setup()? {
        connection
            .execute(setup, &mut DiscardSink)
            .await
            .map_err(|_| McpError::Database)?;
    }
    crate::source::validate(connection, source.as_ref(), engine == ReadEngine::Postgres).await?;
    let command = command(explain_engine, Mode::Plan, sql, Wrap::None);
    let mut rows = Collector::new(plan);
    connection
        .execute(if plan { &command.explain } else { sql }, &mut rows)
        .await
        .map_err(|_| McpError::Database)?;
    let mut bundle = ContextBundle {
        query: Some(sql.into()),
        ..ContextBundle::default()
    };
    if plan {
        if rows.truncated {
            return Err(McpError::Limit);
        }
        bundle.plan = normalize(command.format, &rows.rows, source.as_ref())?;
    } else {
        bundle.samples.push(Sample {
            table: source.map_or_else(
                || "result".into(),
                |(schema, table)| format!("{schema}.{table}"),
            ),
            columns: rows.columns,
            rows: rows.rows,
        });
    }
    Ok(bundle)
}
fn normalize(
    format: openrows_explain::Format,
    rows: &[Vec<Option<String>>],
    source: Option<&(String, String)>,
) -> Result<Vec<Plan>, McpError> {
    let plan = analyse(parse(format, rows).map_err(|_| McpError::Database)?);
    plan.nodes
        .iter()
        .take(200)
        .map(|node| {
            Ok(Plan {
                operation: operation(&node.data.node_type),
                parent: node.parent.and_then(|index| u32::try_from(index).ok()),
                relation: node.data.relation.as_ref().map_or_else(String::new, |_| {
                    source.map_or_else(String::new, |(schema, table)| format!("{schema}.{table}"))
                }),
                predicate: String::new(),
                estimated_rows: estimate(node.data.estimated_rows),
            })
        })
        .collect()
}
fn operation(name: &str) -> PlanOperation {
    let name = name.to_ascii_lowercase();
    if name.contains("scan") {
        PlanOperation::Scan
    } else if name.contains("join") {
        PlanOperation::Join
    } else if name.contains("sort") {
        PlanOperation::Sort
    } else if name.contains("aggregate") {
        PlanOperation::Aggregate
    } else {
        PlanOperation::Other
    }
}
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "planner estimates are clamped to the IPC integer range"
)]
fn estimate(value: Option<f64>) -> u32 {
    value
        .filter(|value| value.is_finite())
        .unwrap_or(0.0)
        .clamp(0.0, f64::from(u32::MAX)) as u32
}
struct Interrupt(Option<Arc<dyn CancelHandle>>);
impl Drop for Interrupt {
    fn drop(&mut self) {
        if let Some(cancel) = self.0.take() {
            tokio::spawn(async move {
                let _ = tokio::time::timeout(Duration::from_secs(1), cancel.cancel()).await;
            });
        }
    }
}
