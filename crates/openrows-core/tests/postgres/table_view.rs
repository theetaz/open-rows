// SPDX-License-Identifier: Apache-2.0
//! D-08 table browsing against a real server: structure, pages, filters
//! with bind parameters (including an injection attempt), sort and count.
use super::{TestResult, config, open};
use arrow_array::{Array, Int32Array};
use arrow_ipc::reader::StreamReader;
use openrows_core::SessionManager;
use openrows_core::script::{ScriptCanceller, ScriptOutcome, run_bound};
use openrows_driver_api::Session;
use openrows_driver_api::{ResultSink, SinkFlow};
use openrows_driver_postgres::PgSession;
use openrows_results::{ResultStore, Window};
use openrows_sql::Dialect;
use openrows_sql::select::{
    BoundSql, Combinator, Filter, FilterOp, Page, Sort, SortDirection, TableQuery, TableRef,
    count_sql, page_sql,
};
use openrows_types::SessionId;
use std::io::Cursor;
use std::sync::Arc;

const ATTACK: &str = "O'Brien'; DROP TABLE d08.\"Users\"; --";

const FIXTURE: [&str; 6] = [
    "DROP SCHEMA IF EXISTS d08 CASCADE",
    "CREATE SCHEMA d08",
    "CREATE TABLE d08.\"Users\" (id int PRIMARY KEY, name text NOT NULL DEFAULT 'anon', note text)",
    "CREATE UNIQUE INDEX users_name_lower ON d08.\"Users\" (lower(name)) WHERE note IS NOT NULL",
    "INSERT INTO d08.\"Users\" SELECT g, 'user ' || g, CASE WHEN g % 10 = 0 THEN NULL ELSE 'n' END \
     FROM generate_series(1, 650) AS g",
    "INSERT INTO d08.\"Users\" VALUES (1000, 'O''Brien''; DROP TABLE d08.\"Users\"; --', 'quote')",
];

struct Discard;

impl ResultSink for Discard {
    fn begin_result(&mut self, _schema: arrow_schema::SchemaRef) {}
    fn push_batch(&mut self, _batch: arrow_array::RecordBatch) -> SinkFlow {
        SinkFlow::Continue
    }
}

fn known() -> Vec<String> {
    ["id", "name", "note"].map(str::to_owned).to_vec()
}

fn users(filters: Vec<Filter>, sort: Option<Sort>) -> TableQuery {
    TableQuery {
        table: TableRef {
            schema: "d08".to_owned(),
            name: "Users".to_owned(),
        },
        columns: Vec::new(),
        filters,
        groups: Vec::new(),
        combinator: Combinator::And,
        sort,
        row_id: None,
    }
}

fn filter(column: &str, op: FilterOp, value: &str) -> Filter {
    Filter {
        column: column.to_owned(),
        op,
        value: value.to_owned(),
        second: String::new(),
    }
}

fn by_id(direction: SortDirection) -> Sort {
    Sort {
        column: "id".to_owned(),
        direction,
    }
}

/// Run one bound statement and return its stored result id.
async fn fetch(
    manager: &SessionManager,
    session: SessionId,
    statement: BoundSql,
) -> TestResult<String> {
    let mut result = None;
    let outcome = run_bound(
        manager,
        session,
        statement,
        &ScriptCanceller::default(),
        |event| {
            if let openrows_core::QueryEvent::ResultBegin { result: id, .. } = event.event {
                result = Some(id.to_string());
            }
        },
    )
    .await;
    assert_eq!(outcome, ScriptOutcome::Completed);
    Ok(result.ok_or("no result")?)
}

/// The `id` column of a stored page.
fn ids(store: &ResultStore, result: &str) -> TestResult<Vec<i32>> {
    let window = Window {
        row_start: 0,
        row_count: 10_000,
        col_start: 0,
        col_count: 1,
    };
    let mut out = Vec::new();
    for batch in StreamReader::try_new(Cursor::new(store.window(result, window)?), None)? {
        let batch = batch?;
        let column = batch
            .column(0)
            .as_any()
            .downcast_ref::<Int32Array>()
            .ok_or("id is not Int32")?;
        out.extend((0..column.len()).map(|row| column.value(row)));
    }
    Ok(out)
}

async fn page(
    (manager, store, session): (&SessionManager, &ResultStore, SessionId),
    query: &TableQuery,
    offset: u64,
) -> TestResult<Vec<i32>> {
    let result = fetch(
        manager,
        session,
        page_sql(
            Dialect::Postgres,
            query,
            &known(),
            Page { limit: 300, offset },
        )?,
    )
    .await?;
    let rows = ids(store, &result)?;
    store.remove(&result);
    Ok(rows)
}

async fn count(
    (manager, store, session): (&SessionManager, &ResultStore, SessionId),
    query: &TableQuery,
) -> TestResult<Option<i64>> {
    let result = fetch(
        manager,
        session,
        count_sql(Dialect::Postgres, query, &known())?,
    )
    .await?;
    let rows = store.first_integer(&result);
    store.remove(&result);
    Ok(rows)
}

async fn create_fixture() -> TestResult<PgSession> {
    let mut setup = PgSession::connect(&config()?).await?;
    for statement in FIXTURE {
        setup.execute(statement, &mut Discard).await?;
    }
    Ok(setup)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs PostgreSQL; run with `just test-pg`"]
async fn table_view_reads_structure_pages_filters_and_counts() -> TestResult {
    let mut metadata = create_fixture().await?;
    let structure = metadata.table_structure("d08", "Users").await?;
    let names: Vec<&str> = structure.columns.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["id", "name", "note"]);
    let keys: Vec<bool> = structure.columns.iter().map(|c| c.primary_key).collect();
    assert_eq!(keys, [true, false, false]);
    let name = structure.columns.get(1).ok_or("no name column")?;
    assert!(!name.nullable && name.data_type == "text");
    assert_eq!(name.default.as_deref(), Some("'anon'::text"));
    let unique = structure
        .indexes
        .iter()
        .find(|i| i.name == "users_name_lower");
    assert!(unique.is_some_and(|index| index.unique
        && index.columns == "lower(name)"
        && index.method == "btree"
        && index.predicate.as_deref() == Some("note IS NOT NULL")));
    assert!(structure.indexes.iter().any(|index| index.primary));
    assert!(
        metadata
            .table_structure("d08", "missing")
            .await?
            .columns
            .is_empty()
    );

    let store = Arc::new(ResultStore::default());
    let manager = SessionManager::new(store.clone());
    let session = open(&manager).await?;
    let on = (&manager, store.as_ref(), session);

    let ascending = users(Vec::new(), Some(by_id(SortDirection::Ascending)));
    let first = page(on, &ascending, 0).await?;
    assert_eq!(
        (first.len(), first.first(), first.last()),
        (300, Some(&1), Some(&300))
    );
    let third = page(on, &ascending, 600).await?;
    assert_eq!((third.len(), third.last()), (51, Some(&1000)));
    let descending = users(Vec::new(), Some(by_id(SortDirection::Descending)));
    assert_eq!(page(on, &descending, 0).await?.first(), Some(&1000));

    let exact = users(vec![filter("name", FilterOp::Equal, ATTACK)], None);
    assert_eq!(page(on, &exact, 0).await?, [1000]);
    let contains = users(vec![filter("name", FilterOp::Contains, "'; drop")], None);
    assert_eq!(page(on, &contains, 0).await?, [1000]);
    let nulls = users(vec![filter("note", FilterOp::IsNull, "")], None);
    assert_eq!(count(on, &nulls).await?, Some(65));
    let mut range = filter("id", FilterOp::Between, "10");
    range.second = "19".to_owned();
    let listed = filter("id", FilterOp::In, "12, 14, 999");
    assert_eq!(count(on, &users(vec![range, listed], None)).await?, Some(2));
    assert_eq!(count(on, &users(Vec::new(), None)).await?, Some(651));

    metadata
        .execute("DROP SCHEMA d08 CASCADE", &mut Discard)
        .await?;
    manager.close(session).await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs PostgreSQL; run with `just test-pg`"]
async fn table_columns_list_enum_labels_in_sort_order() -> TestResult {
    let mut session = PgSession::connect(&config()?).await?;
    for statement in [
        "DROP SCHEMA IF EXISTS d08e CASCADE",
        "CREATE SCHEMA d08e",
        "CREATE TYPE d08e.mood AS ENUM ('sad', 'a:b,c', 'é!')",
        "ALTER TYPE d08e.mood ADD VALUE 'first' BEFORE 'sad'",
        "CREATE DOMAIN d08e.feeling AS d08e.mood",
        "CREATE TABLE d08e.t (id int PRIMARY KEY, mood d08e.mood, feeling d08e.feeling, done boolean)",
    ] {
        session.execute(statement, &mut Discard).await?;
    }
    let labels: Vec<Option<Vec<String>>> = session
        .table_columns("d08e", "t")
        .await?
        .into_iter()
        .map(|column| column.enum_labels)
        .collect();
    let mood = Some(["first", "sad", "a:b,c", "é!"].map(str::to_owned).to_vec());
    assert_eq!(labels, [None, mood.clone(), mood, None]);
    session
        .execute("DROP SCHEMA d08e CASCADE", &mut Discard)
        .await?;
    Ok(())
}
