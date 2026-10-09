// SPDX-License-Identifier: Apache-2.0
//! Analytical-engine contracts: no unique constraints or persistent processes.
use crate::{
    Failure, Group, Target, ensure, fixture_name,
    sink::{Collect, ping, query, sqlstate},
};
use openrows_driver_api::{DriverError, ProcessSignal, admin, catalog, catalog::ObjectKind};
use std::time::{Duration, Instant};

pub(crate) async fn errors(target: &Target) -> Result<(), Failure> {
    let mut connection = target.connect().await?;
    for (sql, expected) in [
        ("SELEC 1", "42601"),
        ("SELECT * FROM orc_nonexistent_table", "42P01"),
        ("SELECT orc_nonexistent_column", "42703"),
        ("SELECT CAST(NULL AS Int64)", "22018"),
    ] {
        let code = sqlstate(query(connection.as_mut(), sql).await, sql)?;
        ensure(code == expected, || {
            format!("{sql}: {code}, expected {expected}")
        })?;
        ping(connection.as_mut()).await?;
    }
    connection.close().await?;
    Ok(())
}

pub(crate) async fn metadata(target: &Target) -> Result<(), Failure> {
    let table = fixture_name(Group::Metadata, "t");
    let view = fixture_name(Group::Metadata, "v");
    let mut connection = target.connect().await?;
    query(connection.as_mut(), &format!("CREATE TABLE {table} (id Int64, name String, note Nullable(String)) ENGINE=MergeTree ORDER BY id")).await?;
    query(
        connection.as_mut(),
        &format!("CREATE VIEW {view} AS SELECT * FROM {table}"),
    )
    .await?;
    let outcome = async {
        let facet = catalog(connection.as_mut())?;
        let databases = facet.list_databases().await?;
        ensure(databases.iter().any(|d| d.name == target.schema), || {
            "current database missing".to_owned()
        })?;
        let objects = facet
            .list_objects(&target.schema, &[ObjectKind::Table, ObjectKind::View])
            .await?;
        for (name, kind) in [(&table, ObjectKind::Table), (&view, ObjectKind::View)] {
            ensure(
                objects.iter().any(|o| o.name == *name && o.kind == kind),
                || format!("missing {name}"),
            )?;
        }
        let columns = facet.table_columns(&target.schema, &table).await?;
        ensure(
            columns.len() == 3 && columns[0].primary_key && columns[2].nullable,
            || format!("columns: {columns:?}"),
        )?;
        let detail = facet.table_detail(&target.schema, &table).await?;
        ensure(detail.columns.len() == 3 && detail.info.is_some(), || {
            "table detail missing".to_owned()
        })?;
        let index = facet.search_index().await?;
        ensure(index.iter().any(|o| o.name == table), || {
            "search index missing table".to_owned()
        })
    }
    .await;
    query(connection.as_mut(), &format!("DROP VIEW {view}")).await?;
    query(connection.as_mut(), &format!("DROP TABLE {table}")).await?;
    connection.close().await?;
    outcome
}

pub(crate) async fn processes(target: &Target) -> Result<(), Failure> {
    let mut lister = target.connect().await?;
    let mut runner = target.connect().await?;
    let started = Instant::now();
    let mut sink = Collect::default();
    let (executed, signalled) = tokio::join!(
        runner.execute(
            "SELECT count() FROM numbers(1000000000000) /* orc_clickhouse_process */",
            &mut sink
        ),
        async {
            let facet = admin(lister.as_mut())?;
            let mut ticks = tokio::time::interval(Duration::from_millis(50));
            loop {
                ticks.tick().await;
                let list = facet.list_processes().await?;
                if let Some(process) = list.iter().find(|p| {
                    p.query
                        .as_deref()
                        .is_some_and(|q| q.contains("orc_clickhouse_process"))
                }) {
                    ensure(
                        matches!(
                            facet
                                .signal_process(process.id, ProcessSignal::Terminate)
                                .await,
                            Err(DriverError::Unsupported(_))
                        ),
                        || "terminate should be unsupported".to_owned(),
                    )?;
                    facet
                        .signal_process(process.id, ProcessSignal::CancelQuery)
                        .await?;
                    break;
                }
                ensure(started.elapsed() < Duration::from_secs(10), || {
                    "query never appeared".to_owned()
                })?;
            }
            Ok::<_, Failure>(())
        },
    );
    signalled?;
    ensure(sqlstate(executed, "cancelled process")? == "57014", || {
        "wrong cancel code".to_owned()
    })?;
    ping(runner.as_mut()).await?;
    runner.close().await?;
    lister.close().await?;
    Ok(())
}
