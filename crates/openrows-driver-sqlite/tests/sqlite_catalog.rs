// SPDX-License-Identifier: Apache-2.0
//! Schema tree, structure and detail of a real database file.
mod support;

use openrows_driver_api::Catalog;
use openrows_driver_api::catalog::ObjectKind;
use openrows_driver_api::schema::{ConstraintKind, Generated, RelationKind, TriggerTiming};
use support::{TestResult, database};

const SCHEMA: &str = "CREATE TABLE owners (id INTEGER PRIMARY KEY, email TEXT NOT NULL UNIQUE);
    CREATE TABLE [order items] (
        id INTEGER PRIMARY KEY,
        owner INTEGER REFERENCES owners(id),
        qty INTEGER NOT NULL DEFAULT 1,
        total REAL GENERATED ALWAYS AS (qty * 2.5) STORED);
    CREATE INDEX items_owner ON [order items](owner) WHERE owner IS NOT NULL;
    CREATE VIEW big AS SELECT * FROM [order items] WHERE qty > 10;
    CREATE TRIGGER audit BEFORE UPDATE OF qty ON [order items] BEGIN SELECT 1; END";

#[tokio::test]
async fn the_tree_lists_schemas_tables_and_views() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut session = database(&dir, SCHEMA).await?;
    let databases = session.list_databases().await?;
    assert_eq!(
        databases
            .iter()
            .map(|d| d.name.as_str())
            .collect::<Vec<_>>(),
        ["main"]
    );
    let schemas = session.list_schemas().await?;
    assert_eq!(
        schemas.first().map(|s| (s.name.as_str(), s.system)),
        Some(("main", false))
    );
    let tables = session.list_objects("main", &[ObjectKind::Table]).await?;
    let names: Vec<&str> = tables.iter().map(|o| o.name.as_str()).collect();
    assert_eq!(names, ["order items", "owners"]);
    let views = session.list_objects("main", &[ObjectKind::View]).await?;
    assert_eq!(
        views.iter().map(|o| o.kind).collect::<Vec<_>>(),
        [ObjectKind::View]
    );
    assert!(
        session
            .list_objects("main", &[ObjectKind::Function])
            .await?
            .is_empty()
    );
    assert_eq!(session.search_index().await?.len(), 3);
    Ok(())
}

#[tokio::test]
async fn structure_and_detail_describe_keys_indexes_and_triggers() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut session = database(&dir, SCHEMA).await?;
    let structure = session.table_structure("main", "order items").await?;
    let columns: Vec<_> = structure
        .columns
        .iter()
        .map(|c| (c.name.as_str(), c.primary_key, c.nullable))
        .collect();
    assert_eq!(
        columns,
        [
            ("id", true, false),
            ("owner", false, true),
            ("qty", false, false),
            ("total", false, true)
        ]
    );
    let partial = structure
        .indexes
        .iter()
        .find(|i| i.name == "items_owner")
        .ok_or("no index")?;
    assert_eq!(partial.predicate.as_deref(), Some("owner IS NOT NULL"));
    let detail = session.table_detail("main", "order items").await?;
    let kinds: Vec<_> = detail.constraints.iter().map(|c| c.kind).collect();
    assert_eq!(kinds, [ConstraintKind::Primary, ConstraintKind::Foreign]);
    let foreign = detail.constraints[1]
        .references
        .clone()
        .ok_or("no target")?;
    assert_eq!(
        (foreign.table.as_str(), foreign.columns.clone()),
        ("owners", vec!["id".to_owned()])
    );
    assert_eq!(detail.columns[3].generated, Generated::Stored);
    assert_eq!(detail.triggers[0].timing, TriggerTiming::Before);
    let info = detail.info.clone().ok_or("no info")?;
    assert_eq!(info.kind, RelationKind::Table);
    let ddl = info.definition.unwrap_or_default();
    assert!(ddl.starts_with("CREATE TABLE [order items]"), "{ddl}");
    assert!(ddl.contains("CREATE INDEX items_owner") && ddl.contains("CREATE TRIGGER audit"));
    let owners = session.table_detail("main", "owners").await?;
    assert!(
        owners
            .constraints
            .iter()
            .any(|c| c.kind == ConstraintKind::Unique && c.columns == ["email"])
    );
    let view = session
        .table_detail("main", "big")
        .await?
        .info
        .ok_or("no view")?;
    assert_eq!(view.kind, RelationKind::View);
    assert!(
        session
            .table_detail("main", "missing")
            .await?
            .info
            .is_none()
    );
    assert!(session.table_columns("main", "missing").await?.is_empty());
    Ok(())
}

#[tokio::test]
async fn view_inspection_is_read_only_and_optional_catalogs_are_typed_unsupported() -> TestResult {
    use openrows_driver_api::DriverError;
    use openrows_driver_api::inspection::ObjectIdentity;
    let dir = tempfile::tempdir()?;
    let mut session = database(&dir, SCHEMA).await?;
    let view = ObjectIdentity {
        schema: "main".to_owned(),
        name: "big".to_owned(),
        kind: ObjectKind::View,
        arguments: None,
    };
    let inspected = session.object_detail(&view).await?;
    assert!(
        inspected
            .definition
            .as_deref()
            .is_some_and(|sql| sql.starts_with("CREATE VIEW"))
    );
    assert_eq!(inspected.sections[0].rows.len(), 4);
    assert!(matches!(
        session.list_roles().await,
        Err(DriverError::Unsupported(_))
    ));
    assert!(matches!(
        session.list_extensions().await,
        Err(DriverError::Unsupported(_))
    ));
    assert!(matches!(
        session.role_grants(("x", "cluster")).await,
        Err(DriverError::Unsupported(_))
    ));
    let unsupported = ObjectIdentity {
        kind: ObjectKind::Type,
        ..view
    };
    assert!(matches!(
        session.object_detail(&unsupported).await,
        Err(DriverError::Unsupported(_))
    ));
    Ok(())
}
