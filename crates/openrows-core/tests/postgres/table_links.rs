// SPDX-License-Identifier: Apache-2.0
//! Cross-schema FK metadata and the referenced-row query used by grid navigation.
use super::{TestResult, config, open};
use openrows_core::script::{ScriptCanceller, ScriptOutcome, run_bound};
use openrows_core::{QueryEvent, SessionManager};
use openrows_driver_api::{ResultSink, Session, SinkFlow};
use openrows_driver_postgres::PgSession;
use openrows_results::ResultStore;
use openrows_sql::{
    Dialect,
    select::{Combinator, Filter, FilterOp, TableQuery, TableRef, count_sql},
};
use std::sync::Arc;

struct Discard;
impl ResultSink for Discard {
    fn begin_result(&mut self, _schema: arrow_schema::SchemaRef) {}
    fn push_batch(&mut self, _batch: arrow_array::RecordBatch) -> SinkFlow {
        SinkFlow::Continue
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs PostgreSQL; run with `just test-pg`"]
async fn cross_schema_foreign_key_retains_destination_and_reads_referenced_row() -> TestResult {
    let mut metadata = PgSession::connect(&config()?).await?;
    for sql in [
        "CREATE SCHEMA a",
        "CREATE SCHEMA b",
        "CREATE TABLE b.customers (id int PRIMARY KEY)",
        "CREATE TABLE a.customers (id int PRIMARY KEY)",
        "CREATE TABLE a.orders (id int PRIMARY KEY, customer_id int REFERENCES b.customers(id))",
        "INSERT INTO b.customers VALUES (7)",
        "INSERT INTO a.customers VALUES (99)",
        "INSERT INTO a.orders VALUES (1, 7)",
    ] {
        metadata.execute(sql, &mut Discard).await?;
    }
    let detail = metadata.table_detail("a", "orders").await?;
    let key = detail
        .constraints
        .iter()
        .find(|key| key.references.is_some())
        .ok_or("no FK")?;
    assert_eq!(key.columns, ["customer_id"]);
    let target = key.references.as_ref().ok_or("no target")?;
    assert_eq!((&*target.schema, &*target.table), ("b", "customers"));
    assert_eq!(target.columns, ["id"]);
    let query = TableQuery {
        table: TableRef {
            schema: target.schema.clone(),
            name: target.table.clone(),
        },
        columns: Vec::new(),
        groups: Vec::new(),
        combinator: Combinator::And,
        sort: None,
        row_id: None,
        filters: vec![Filter {
            column: target.columns[0].clone(),
            op: FilterOp::Equal,
            value: "7".into(),
            second: String::new(),
        }],
    };
    let store = Arc::new(ResultStore::default());
    let manager = SessionManager::new(store.clone());
    let session = open(&manager).await?;
    let mut result = None;
    let outcome = run_bound(
        &manager,
        session,
        count_sql(Dialect::Postgres, &query, &["id".into()])?,
        &ScriptCanceller::default(),
        |event| {
            if let QueryEvent::ResultBegin { result: id, .. } = event.event {
                result = Some(id.to_string());
            }
        },
    )
    .await;
    assert_eq!(outcome, ScriptOutcome::Completed);
    let result = result.ok_or("no row result")?;
    assert_eq!(store.first_integer(&result), Some(1));
    store.remove(&result);
    manager.close(session).await?;
    for sql in ["DROP SCHEMA a CASCADE", "DROP SCHEMA b CASCADE"] {
        metadata.execute(sql, &mut Discard).await?;
    }
    Ok(())
}
