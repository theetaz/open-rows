// SPDX-License-Identifier: Apache-2.0
//! Catalog listing for the schema tree against a real server.
use crate::support::{Collect, TestResult, connect, run};
use openrows_driver_api::catalog::ObjectKind;

/// One statement each: the driver executes through the extended protocol.
const FIXTURE: [&str; 11] = [
    "DROP SCHEMA IF EXISTS d06 CASCADE",
    "CREATE SCHEMA d06",
    "COMMENT ON SCHEMA d06 IS 'schema tree fixture'",
    "CREATE TABLE d06.orders (id int PRIMARY KEY, total numeric)",
    "INSERT INTO d06.orders SELECT g, g FROM generate_series(1, 500) AS g",
    "ANALYZE d06.orders",
    "CREATE VIEW d06.big_orders AS SELECT * FROM d06.orders WHERE total > 100",
    "CREATE MATERIALIZED VIEW d06.totals AS SELECT sum(total) FROM d06.orders",
    "CREATE SEQUENCE d06.ticket_seq",
    "CREATE FUNCTION d06.tax(amount numeric) RETURNS numeric LANGUAGE sql AS 'SELECT amount * 0.2'",
    "CREATE PROCEDURE d06.archive() LANGUAGE sql AS 'SELECT 1'",
];

#[tokio::test]
#[ignore = "needs PostgreSQL: run `just test-pg`"]
async fn introspector_lists_databases_schemas_and_objects() -> TestResult {
    let mut session = connect().await?;
    for statement in FIXTURE {
        let (created, _) = run(&mut session, statement, Collect::default()).await;
        created?;
    }

    let databases = session.list_databases().await?;
    let postgres = databases
        .iter()
        .find(|database| database.name == "postgres");
    assert!(
        postgres.is_some_and(|database| database.system),
        "{databases:?}"
    );
    assert!(
        databases
            .iter()
            .all(|database| !database.name.starts_with("template"))
    );

    let schemas = session.list_schemas().await?;
    let fixture = schemas.iter().find(|schema| schema.name == "d06");
    assert!(fixture.is_some_and(|schema| !schema.system && schema.comment.is_some()));
    assert!(
        schemas
            .iter()
            .any(|schema| schema.name == "pg_catalog" && schema.system)
    );
    assert!(
        schemas
            .iter()
            .any(|schema| schema.name == "pg_toast" && schema.system)
    );

    let objects = session.list_objects("d06", &ObjectKind::ALL).await?;
    let listed: Vec<(&str, ObjectKind)> = objects
        .iter()
        .map(|object| (object.name.as_str(), object.kind))
        .collect();
    assert_eq!(
        listed,
        [
            ("archive", ObjectKind::Procedure),
            ("big_orders", ObjectKind::View),
            ("orders", ObjectKind::Table),
            ("tax", ObjectKind::Function),
            ("ticket_seq", ObjectKind::Sequence),
            ("totals", ObjectKind::MaterializedView),
        ]
    );
    let orders = objects.iter().find(|object| object.name == "orders");
    assert_eq!(orders.and_then(|table| table.row_estimate), Some(500));
    let tax = objects.iter().find(|object| object.name == "tax");
    assert_eq!(
        tax.and_then(|f| f.arguments.as_deref()),
        Some("amount numeric")
    );

    let views = session.list_objects("d06", &[ObjectKind::View]).await?;
    assert_eq!(views.len(), 1);
    let index = session.search_index().await?;
    assert!(
        index
            .iter()
            .any(|entry| entry.schema == "d06" && entry.name == "orders")
    );
    assert!(index.iter().all(|entry| entry.schema != "pg_catalog"));

    let (dropped, _) = run(&mut session, "DROP SCHEMA d06 CASCADE", Collect::default()).await;
    dropped?;
    Ok(())
}

#[tokio::test]
#[ignore = "needs PostgreSQL: run `just test-pg`"]
async fn sidebar_system_visibility_keeps_pg_catalog_hidden_until_enabled() -> TestResult {
    let mut session = connect().await?;
    let schemas = session.list_schemas().await?;
    let objects = session
        .list_objects("pg_catalog", &[ObjectKind::Table])
        .await?;
    assert!(objects.iter().any(|object| object.name == "pg_class"));
    for show_system in [false, true] {
        let visible: Vec<_> = objects
            .iter()
            .filter(|object| {
                schemas
                    .iter()
                    .any(|schema| schema.name == object.schema && (show_system || !schema.system))
            })
            .collect();
        assert_eq!(
            visible.iter().any(|object| object.name == "pg_class"),
            show_system
        );
    }
    Ok(())
}

#[tokio::test]
#[ignore = "needs PostgreSQL: run `just test-pg`"]
async fn toast_tables_are_browsable_when_system_objects_are_enabled() -> TestResult {
    let mut session = connect().await?;
    let objects = session
        .list_objects("pg_toast", &[ObjectKind::Table])
        .await?;
    let table = objects
        .first()
        .ok_or("PostgreSQL catalogs have TOAST tables")?;
    assert_eq!(table.kind, ObjectKind::Table);
    assert!(
        !session
            .table_columns("pg_toast", &table.name)
            .await?
            .is_empty()
    );
    let detail = session.table_detail("pg_toast", &table.name).await?;
    assert!(
        detail
            .info
            .is_some_and(|info| { info.kind == openrows_driver_api::schema::RelationKind::Table })
    );
    assert_eq!(detail.columns.len(), 3);
    assert!(
        session
            .search_index()
            .await?
            .iter()
            .all(|object| object.schema != "pg_toast")
    );
    Ok(())
}
