// SPDX-License-Identifier: Apache-2.0
use super::support::{Collect, TestResult, config};
use openrows_driver_api::{
    ConnectTrace, Connection, Session, catalog::ObjectKind, inspection::ObjectIdentity,
};
use openrows_driver_clickhouse::connect;

#[tokio::test]
#[ignore = "needs ClickHouse; run just test-clickhouse"]
async fn catalog_inspects_views_functions_indexes_and_sizes() -> TestResult {
    let mut session = connect(&config()?, &mut ConnectTrace::default()).await?;
    let table = format!("or_ch_{}", uuid::Uuid::now_v7().simple());
    let view = format!("{table}_v");
    let materialized = format!("{table}_m");
    let function = format!("{table}_f");
    for sql in [
        format!(
            "CREATE TABLE {table} (id Int64, INDEX ids id TYPE minmax GRANULARITY 1) ENGINE=MergeTree ORDER BY id"
        ),
        format!("CREATE VIEW {view} AS SELECT id FROM {table}"),
        format!("CREATE MATERIALIZED VIEW {materialized} ENGINE=Memory AS SELECT id FROM {table}"),
        format!("CREATE FUNCTION {function} AS (x) -> x + 1"),
    ] {
        session.execute(&sql, &mut Collect::default()).await?;
    }
    let outcome = inspect(&mut session, [&table, &view, &materialized, &function]).await;
    for sql in [
        format!("DROP VIEW {materialized}"),
        format!("DROP VIEW {view}"),
        format!("DROP TABLE {table}"),
        format!("DROP FUNCTION {function}"),
    ] {
        session.execute(&sql, &mut Collect::default()).await?;
    }
    outcome
}

async fn inspect(session: &mut dyn Connection, names: [&str; 4]) -> TestResult {
    let [table, view, materialized, function] = names;
    let summary = session
        .execute(
            &format!("INSERT INTO {table} VALUES (1), (2)"),
            &mut Collect::default(),
        )
        .await?;
    // The HTTP summary includes the materialized view's two written rows.
    assert_eq!(summary.command_tag, "INSERT 4");
    let catalog = session.catalog().ok_or("no catalog")?;
    let objects = catalog
        .list_objects(
            "openrows_test",
            &[
                ObjectKind::Table,
                ObjectKind::View,
                ObjectKind::MaterializedView,
                ObjectKind::Function,
            ],
        )
        .await?;
    for (name, kind) in [
        (table, ObjectKind::Table),
        (view, ObjectKind::View),
        (materialized, ObjectKind::MaterializedView),
        (function, ObjectKind::Function),
    ] {
        assert!(
            objects
                .iter()
                .any(|object| object.name == name && object.kind == kind)
        );
    }
    let structure = catalog.table_structure("openrows_test", table).await?;
    assert_eq!(structure.indexes[0].name, "ids");
    assert_eq!(structure.indexes[0].method, "minmax");
    let detail = catalog.table_detail("openrows_test", table).await?;
    let info = detail.info.ok_or("no table info")?;
    assert_eq!(info.row_estimate, Some(2));
    assert!(info.total_bytes > 0);
    for (name, kind) in [
        (view, ObjectKind::View),
        (materialized, ObjectKind::MaterializedView),
        (function, ObjectKind::Function),
    ] {
        let detail = catalog
            .object_detail(&ObjectIdentity {
                schema: "openrows_test".to_owned(),
                name: name.to_owned(),
                kind,
                arguments: None,
            })
            .await?;
        assert!(detail.definition.is_some());
    }
    Ok(())
}
