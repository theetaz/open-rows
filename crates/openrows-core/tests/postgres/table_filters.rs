// SPDX-License-Identifier: Apache-2.0
//! M2-19 filter builder against a real server: a raw SQL predicate beside
//! bound filters, OR, regex and a composite foreign-key jump, counted.
use super::{TestResult, open};
use openrows_core::script::{ScriptCanceller, ScriptOutcome, run_bound};
use openrows_core::{QueryEvent, SessionManager};
use openrows_results::ResultStore;
use openrows_sql::Dialect;
use openrows_sql::select::{
    BoundSql, Combinator, Filter, FilterGroup, FilterOp, TableQuery, TableRef, count_sql,
};
use openrows_types::SessionId;
use std::sync::Arc;

const FIXTURE: [&str; 4] = [
    "DROP SCHEMA IF EXISTS m219 CASCADE",
    "CREATE SCHEMA m219",
    "CREATE TABLE m219.items (id int PRIMARY KEY, org int, region text, name text)",
    "INSERT INTO m219.items SELECT g, g % 5, CASE WHEN g % 2 = 0 THEN 'eu' ELSE 'us' END, \
     'item ' || g FROM generate_series(1, 100) AS g",
];

fn known() -> Vec<String> {
    ["id", "org", "region", "name"].map(str::to_owned).to_vec()
}

fn filter(column: &str, op: FilterOp, value: &str) -> Filter {
    Filter {
        column: column.to_owned(),
        op,
        value: value.to_owned(),
        second: String::new(),
    }
}

fn items(filters: Vec<Filter>, combinator: Combinator) -> TableQuery {
    TableQuery {
        table: TableRef {
            schema: "m219".to_owned(),
            name: "items".to_owned(),
        },
        columns: Vec::new(),
        filters,
        groups: Vec::new(),
        combinator,
        sort: None,
        row_id: None,
    }
}

async fn run(manager: &SessionManager, session: SessionId, sql: &str) -> TestResult {
    let statement = BoundSql {
        sql: sql.to_owned(),
        params: Vec::new(),
    };
    let outcome = run_bound(
        manager,
        session,
        statement,
        &ScriptCanceller::default(),
        |_| {},
    )
    .await;
    assert_eq!(outcome, ScriptOutcome::Completed, "{sql}");
    Ok(())
}

async fn count(
    (manager, store, session): (&SessionManager, &ResultStore, SessionId),
    query: &TableQuery,
) -> TestResult<Option<i64>> {
    let mut result = None;
    let statement = count_sql(Dialect::Postgres, query, &known())?;
    let outcome = run_bound(
        manager,
        session,
        statement,
        &ScriptCanceller::default(),
        |event| {
            if let QueryEvent::ResultBegin { result: id, .. } = event.event {
                result = Some(id.to_string());
            }
        },
    )
    .await;
    assert_eq!(outcome, ScriptOutcome::Completed);
    let id = result.ok_or("no result")?;
    let rows = store.first_integer(&id);
    store.remove(&id);
    Ok(rows)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs PostgreSQL; run with `just test-pg`"]
async fn raw_predicates_and_key_jumps_filter_rows() -> TestResult {
    let store = Arc::new(ResultStore::default());
    let manager = SessionManager::new(store.clone());
    let session = open(&manager).await?;
    for statement in FIXTURE {
        run(&manager, session, statement).await?;
    }
    let on = (&manager, store.as_ref(), session);
    let raw = filter("", FilterOp::Raw, "id % 10 = 0 AND (name LIKE 'item %')");
    assert_eq!(
        count(on, &items(vec![raw.clone()], Combinator::And)).await?,
        Some(10)
    );
    let either = vec![raw, filter("id", FilterOp::LessOrEqual, "3")];
    assert_eq!(count(on, &items(either, Combinator::Or)).await?, Some(13));
    let regex = filter("name", FilterOp::Regex, "^item 9[0-9]$");
    assert_eq!(
        count(on, &items(vec![regex], Combinator::And)).await?,
        Some(10)
    );
    let jump = vec![
        filter("org", FilterOp::Equal, "2"),
        filter("region", FilterOp::Equal, "eu"),
    ];
    assert_eq!(count(on, &items(jump, Combinator::And)).await?, Some(10));
    run(&manager, session, "DROP SCHEMA m219 CASCADE").await?;
    manager.close(session).await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs PostgreSQL; run with `just test-pg`"]
async fn grouped_and_or_predicates_filter_rows_with_bind_parameters() -> TestResult {
    let store = Arc::new(ResultStore::default());
    let manager = SessionManager::new(store.clone());
    let session = open(&manager).await?;
    run(&manager, session, "CREATE SCHEMA fu03_groups").await?;
    run(
        &manager,
        session,
        "CREATE TABLE fu03_groups.items (id int, org int, region text, name text)",
    )
    .await?;
    run(&manager, session, "INSERT INTO fu03_groups.items SELECT g, g % 5, 'eu', 'item ' || g FROM generate_series(1, 100) g").await?;
    let mut query = items(Vec::new(), Combinator::And);
    query.table.schema = "fu03_groups".into();
    query.groups = vec![
        FilterGroup {
            filters: vec![
                filter("id", FilterOp::LessOrEqual, "3"),
                filter("id", FilterOp::GreaterOrEqual, "98"),
            ],
            combinator: Combinator::Or,
        },
        FilterGroup {
            filters: vec![
                filter("", FilterOp::Raw, "id % 2 = 0"),
                filter("region", FilterOp::Equal, "eu"),
            ],
            combinator: Combinator::And,
        },
    ];
    let on = (&manager, store.as_ref(), session);
    assert_eq!(count(on, &query).await?, Some(3));
    query.combinator = Combinator::Or;
    assert_eq!(count(on, &query).await?, Some(53));
    run(&manager, session, "DROP SCHEMA fu03_groups CASCADE").await?;
    manager.close(session).await?;
    Ok(())
}
