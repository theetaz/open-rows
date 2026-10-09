// SPDX-License-Identifier: Apache-2.0
//! Import scenarios every engine runs: values and quoting, conflict
//! policies and the dry run, bad rows, per-batch commits, table creation,
//! truncation and cancel.
use openrows_driver_api::Connection;
use openrows_import::{
    BadRows, CancellationToken, ConflictPolicy, Control, Failure, NewColumn, Outcome, RunMode,
    SampleLimits, TableName, TransactionMode, TypeCategory, create_table_sql, inspect, load,
    spawn_feed,
};
use openrows_sql::Dialect;

use super::{TestResult, count, finished, identity, import, plan, run, write};

/// One engine: its dialect, the schema tables live in and its DDL types.
pub struct Engine {
    pub dialect: Dialect,
    pub schema: &'static str,
    /// Column definitions of `(id, name, score, active, joined)`.
    pub columns: &'static str,
}

const PEOPLE: &str = "id,name,score,active,joined\n\
    1,\"Ada, Countess\",1.5,yes,2024-01-02\n\
    2,\"multi\nline \"\"q\"\"\",,no,2024-01-03\n\
    3,plain,-2,true,\n";

fn mapping() -> Vec<openrows_import::MappedColumn> {
    identity(&[
        ("id", TypeCategory::Integer),
        ("name", TypeCategory::Text),
        ("score", TypeCategory::Decimal),
        ("active", TypeCategory::Boolean),
        ("joined", TypeCategory::Date),
    ])
}

async fn fresh(connection: &mut dyn Connection, engine: &Engine, table: &str) -> TestResult {
    run(connection, &format!("DROP TABLE IF EXISTS {table}")).await?;
    run(
        connection,
        &format!("CREATE TABLE {table} {}", engine.columns),
    )
    .await
}

/// Typed values, quoting, embedded newlines, NULLs and the fast path.
pub async fn values_round_trip(connection: &mut dyn Connection, engine: &Engine) -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = write(&dir, "people.csv", PEOPLE)?;
    fresh(connection, engine, "imp_values").await?;
    let plan = plan(engine.dialect, (engine.schema, "imp_values"), &mapping());
    let summary = finished(import(connection, &path, (mapping(), &plan), 2).await?)?;
    assert_eq!((summary.rows, summary.inserted, summary.failed), (3, 3, 0));
    for (condition, expected) in [
        ("", 3),
        (
            "WHERE id = 1 AND name = 'Ada, Countess' AND score = 1.5 AND active AND joined = '2024-01-02'",
            1,
        ),
        (
            "WHERE id = 2 AND name LIKE 'multi%q\"' AND score IS NULL AND NOT active",
            1,
        ),
        ("WHERE id = 3 AND joined IS NULL AND score = -2", 1),
    ] {
        let sql = format!("SELECT 1 FROM imp_values {condition}");
        assert_eq!(count(connection, &sql).await?, expected, "{sql}");
    }
    Ok(())
}

/// A table holding ids 1 and 2 named `old`, and a keyed plan into it.
async fn keyed(
    connection: &mut dyn Connection,
    engine: &Engine,
    table: &str,
) -> TestResult<openrows_import::LoadPlan> {
    fresh(connection, engine, table).await?;
    let insert = format!("INSERT INTO {table} (id, name) VALUES (1, 'old'), (2, 'old')");
    run(connection, &insert).await?;
    let mut plan = plan(engine.dialect, (engine.schema, table), &mapping());
    plan.key = vec!["id".to_owned()];
    Ok(plan)
}

/// The dry run counts existing keys and writes nothing; the error policy fails.
pub async fn conflict_check(connection: &mut dyn Connection, engine: &Engine) -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = write(&dir, "people.csv", PEOPLE)?;
    let table = "imp_conflict_check";
    let mut plan = keyed(connection, engine, table).await?;
    plan.mode = RunMode::Check;
    let checked = finished(import(connection, &path, (mapping(), &plan), 2).await?)?;
    assert_eq!(
        (checked.rows, checked.conflicts, checked.inserted),
        (3, Some(2), 0)
    );
    let all = format!("SELECT 1 FROM {table}");
    assert_eq!(count(connection, &all).await?, 2);
    plan.mode = RunMode::Import;
    let failed = import(connection, &path, (mapping(), &plan), 10).await?;
    assert!(matches!(failed, Outcome::Failed(Failure::Batch { .. }, ref s) if s.inserted == 0));
    assert_eq!(count(connection, &all).await?, 2);
    Ok(())
}

/// Skip keeps existing rows; update replaces them.
pub async fn conflict_policies(connection: &mut dyn Connection, engine: &Engine) -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = write(&dir, "people.csv", PEOPLE)?;
    let table = "imp_conflict_policies";
    let mut plan = keyed(connection, engine, table).await?;
    plan.conflict = ConflictPolicy::Skip;
    let skipped = finished(import(connection, &path, (mapping(), &plan), 10).await?)?;
    // MySQL reports a kept duplicate as a found row (CLIENT_FOUND_ROWS).
    let expected = if engine.dialect == Dialect::MySql {
        (3, 0)
    } else {
        (1, 2)
    };
    assert_eq!((skipped.inserted, skipped.skipped), expected);
    let old = format!("SELECT 1 FROM {table} WHERE name = 'old'");
    assert_eq!(count(connection, &old).await?, 2);
    plan.conflict = ConflictPolicy::Update;
    let updated = finished(import(connection, &path, (mapping(), &plan), 10).await?)?;
    assert_eq!(updated.inserted, 3);
    assert_eq!(count(connection, &old).await?, 0);
    assert_eq!(
        count(connection, &format!("SELECT 1 FROM {table}")).await?,
        3
    );
    Ok(())
}

/// Bad values are skipped with their line, or stop and roll back everything.
pub async fn bad_rows(connection: &mut dyn Connection, engine: &Engine) -> TestResult {
    let dir = tempfile::tempdir()?;
    let text = "id,name,score,active,joined\n1,a,1,true,2024-01-01\nx,b,2,true,2024-01-01\n3,c,3,maybe,2024-01-01\n4,d,4,false,2024-01-01\n";
    let path = write(&dir, "bad.csv", text)?;
    let table = "imp_bad";
    fresh(connection, engine, table).await?;
    let mut plan = plan(engine.dialect, (engine.schema, table), &mapping());
    let summary = finished(import(connection, &path, (mapping(), &plan), 10).await?)?;
    assert_eq!((summary.inserted, summary.failed), (2, 2));
    let lines: Vec<(u64, Option<&str>)> = summary
        .errors
        .iter()
        .map(|error| (error.line, error.column.as_deref()))
        .collect();
    assert_eq!(lines, [(3, Some("id")), (4, Some("active"))]);

    fresh(connection, engine, table).await?;
    plan.bad_rows = BadRows::Abort;
    let outcome = import(connection, &path, (mapping(), &plan), 1).await?;
    assert!(matches!(outcome, Outcome::Failed(Failure::BadRow, ref s) if s.inserted == 0));
    assert_eq!(
        count(connection, &format!("SELECT 1 FROM {table}")).await?,
        0
    );
    Ok(())
}

/// Per-batch commits keep the batches before a failing one.
pub async fn per_batch_commits(connection: &mut dyn Connection, engine: &Engine) -> TestResult {
    let dir = tempfile::tempdir()?;
    let text = "id,name,score,active,joined\n1,a,1,t,2024-01-01\n2,b,1,t,2024-01-01\n3,c,1,t,2024-01-01\n1,dup,1,t,2024-01-01\n";
    let path = write(&dir, "dup.csv", text)?;
    let table = "imp_batches";
    fresh(connection, engine, table).await?;
    let mut plan = plan(engine.dialect, (engine.schema, table), &mapping());
    plan.transaction = TransactionMode::PerBatch;
    let outcome = import(connection, &path, (mapping(), &plan), 2).await?;
    let Outcome::Failed(Failure::Batch { lines, .. }, summary) = outcome else {
        return Err(format!("expected a failed batch: {outcome:?}").into());
    };
    assert_eq!((lines, summary.inserted), (Some((4, 5)), 2));
    assert_eq!(
        count(connection, &format!("SELECT 1 FROM {table}")).await?,
        2
    );
    Ok(())
}

/// A new table from inferred types, then truncate-first replaces its rows.
pub async fn create_and_truncate(connection: &mut dyn Connection, engine: &Engine) -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = write(&dir, "people.csv", PEOPLE)?;
    let table = "imp_created";
    run(connection, &format!("DROP TABLE IF EXISTS {table}")).await?;
    let found = inspect(
        &path,
        None,
        SampleLimits {
            preview: 5,
            sample: 50,
        },
    )?;
    let columns: Vec<NewColumn> = found
        .columns
        .iter()
        .map(|column| NewColumn {
            name: column.name.clone(),
            data_type: column
                .stats
                .category()
                .default_type(engine.dialect, column.stats.longest)
                .to_owned(),
        })
        .collect();
    let name = TableName {
        schema: engine.schema.to_owned(),
        table: table.to_owned(),
    };
    let mut plan = plan(engine.dialect, (engine.schema, table), &mapping());
    plan.create = Some(create_table_sql(
        engine.dialect,
        &name,
        &columns,
        &["id".to_owned()],
    )?);
    let created = finished(import(connection, &path, (mapping(), &plan), 2).await?)?;
    assert_eq!(created.inserted, 3);
    plan.create = None;
    plan.truncate = true;
    finished(import(connection, &path, (mapping(), &plan), 2).await?)?;
    assert_eq!(
        count(connection, &format!("SELECT 1 FROM {table}")).await?,
        3
    );

    // A failed import into a new table leaves no table behind.
    run(connection, &format!("DROP TABLE {table}")).await?;
    let bad = write(
        &dir,
        "dup.csv",
        "id,name,score,active,joined\n1,a,1,t,\n1,b,1,t,\n",
    )?;
    plan.truncate = false;
    plan.create = Some(create_table_sql(
        engine.dialect,
        &name,
        &columns,
        &["id".to_owned()],
    )?);
    let failed = import(connection, &bad, (mapping(), &plan), 10).await?;
    assert!(matches!(failed, Outcome::Failed(Failure::Batch { .. }, _)));
    let probe = format!("SELECT 1 FROM {table}");
    assert!(
        count(connection, &probe).await.is_err(),
        "the table was rolled back"
    );
    Ok(())
}

/// A cancelled import writes nothing and ends `Cancelled`.
pub async fn cancel(connection: &mut dyn Connection, engine: &Engine) -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = write(&dir, "people.csv", PEOPLE)?;
    let table = "imp_cancel";
    fresh(connection, engine, table).await?;
    let plan = plan(engine.dialect, (engine.schema, table), &mapping());
    let found = inspect(
        &path,
        None,
        SampleLimits {
            preview: 5,
            sample: 50,
        },
    )?;
    let columns: Vec<String> = found.columns.iter().map(|c| c.name.clone()).collect();
    let mapper = openrows_import::Mapper::new(mapping(), engine.dialect, columns.len())?;
    let feed = spawn_feed(
        openrows_import::FeedSource {
            path,
            options: found.options,
            columns,
            mapper,
            batch_rows: 1,
        },
        std::sync::Arc::default(),
    );
    let cancel = CancellationToken::new();
    cancel.cancel();
    let control = Control {
        cancel,
        progress: Box::new(|_| {}),
    };
    let outcome = load(connection, &plan, feed, control).await;
    assert!(matches!(outcome, Outcome::Cancelled(ref s) if s.inserted == 0));
    assert_eq!(
        count(connection, &format!("SELECT 1 FROM {table}")).await?,
        0
    );
    Ok(())
}

/// Every scenario, in order.
pub async fn all(connection: &mut dyn Connection, engine: &Engine) -> TestResult {
    values_round_trip(connection, engine).await?;
    conflict_check(connection, engine).await?;
    conflict_policies(connection, engine).await?;
    bad_rows(connection, engine).await?;
    per_batch_commits(connection, engine).await?;
    create_and_truncate(connection, engine).await?;
    cancel(connection, engine).await
}
