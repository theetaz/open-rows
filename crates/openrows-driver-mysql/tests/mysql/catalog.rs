// SPDX-License-Identifier: Apache-2.0
//! The schema tree and table detail from `information_schema`.
use openrows_driver_api::Catalog;
use openrows_driver_api::catalog::ObjectKind;
use openrows_driver_api::schema::{ConstraintKind, Generated, RelationKind};

use super::support::{DATABASE, TestResult, run, session};

/// Two tables, a key, a foreign key, a generated column and a view, with
/// names prefixed so tests can run side by side.
fn schema(prefix: &str) -> Vec<String> {
    vec![
        format!("DROP VIEW IF EXISTS {prefix}_big"),
        format!("DROP TABLE IF EXISTS {prefix}_lines"),
        format!("DROP TABLE IF EXISTS {prefix}_owners"),
        format!(
            "CREATE TABLE {prefix}_owners (id INT PRIMARY KEY, email VARCHAR(80) NOT NULL UNIQUE) \
             COMMENT 'people'"
        ),
        format!(
            "CREATE TABLE {prefix}_lines (id INT AUTO_INCREMENT PRIMARY KEY, owner INT, \
             qty INT NOT NULL DEFAULT 1, mood ENUM('ok','bad'), total INT AS (qty * 2) STORED, \
             KEY by_owner (owner), CONSTRAINT {prefix}_fk FOREIGN KEY (owner) REFERENCES {prefix}_owners (id))"
        ),
        format!("CREATE VIEW {prefix}_big AS SELECT * FROM {prefix}_lines WHERE qty > 10"),
    ]
}

async fn create(
    session: &mut openrows_driver_mysql::MySqlSession,
    prefix: &str,
) -> Result<(), openrows_driver_api::DriverError> {
    let statements = schema(prefix);
    let borrowed: Vec<&str> = statements.iter().map(String::as_str).collect();
    run(session, &borrowed).await
}

#[tokio::test]
#[ignore = "needs MySQL or MariaDB; run with `just test-mysql`"]
async fn the_tree_lists_databases_schemas_and_objects() -> TestResult {
    let mut session = session().await?;
    create(&mut session, "tree").await?;
    let databases = session.list_databases().await?;
    assert!(databases.iter().any(|d| d.name == DATABASE && !d.system));
    assert!(
        databases
            .iter()
            .any(|d| d.name == "information_schema" && d.system)
    );
    let schemas = session.list_schemas().await?;
    assert_eq!(
        schemas.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
        [DATABASE]
    );
    let tables = session.list_objects(DATABASE, &[ObjectKind::Table]).await?;
    let names: Vec<&str> = tables.iter().map(|o| o.name.as_str()).collect();
    assert!(names.contains(&"tree_lines") && names.contains(&"tree_owners"));
    assert!(!names.contains(&"tree_big"));
    let views = session.list_objects(DATABASE, &[ObjectKind::View]).await?;
    assert!(views.iter().any(|o| o.name == "tree_big"));
    assert!(session.search_index().await?.len() >= 3);
    Ok(())
}

#[tokio::test]
#[ignore = "needs MySQL or MariaDB; run with `just test-mysql`"]
async fn structure_and_detail_describe_keys_and_definitions() -> TestResult {
    let mut session = session().await?;
    create(&mut session, "detail").await?;
    let structure = session.table_structure(DATABASE, "detail_lines").await?;
    let id = structure.columns.first().ok_or("no columns")?;
    assert!(id.primary_key && !id.nullable);
    let mood = structure
        .columns
        .iter()
        .find(|c| c.name == "mood")
        .ok_or("no mood")?;
    assert_eq!(
        mood.enum_labels,
        Some(vec!["ok".to_owned(), "bad".to_owned()])
    );
    assert!(
        structure
            .indexes
            .iter()
            .any(|i| i.name == "PRIMARY" && i.primary)
    );
    let detail = session.table_detail(DATABASE, "detail_lines").await?;
    let foreign = detail
        .constraints
        .iter()
        .find(|c| c.kind == ConstraintKind::Foreign)
        .ok_or("no fk")?;
    assert_eq!(
        foreign.references.as_ref().map(|t| t.table.as_str()),
        Some("detail_owners")
    );
    let total = detail
        .columns
        .iter()
        .find(|c| c.name == "total")
        .ok_or("no total")?;
    assert_eq!(total.generated, Generated::Stored);
    let info = detail.info.clone().ok_or("no info")?;
    assert_eq!(info.kind, RelationKind::Table);
    assert!(
        info.definition
            .unwrap_or_default()
            .starts_with("CREATE TABLE `detail_lines`")
    );
    let owners = session.table_detail(DATABASE, "detail_owners").await?;
    assert!(
        owners
            .constraints
            .iter()
            .any(|c| c.kind == ConstraintKind::Unique && c.columns == ["email"])
    );
    assert_eq!(
        owners.info.and_then(|i| i.comment).as_deref(),
        Some("people")
    );
    let view = session
        .table_detail(DATABASE, "detail_big")
        .await?
        .info
        .ok_or("no view")?;
    assert_eq!(view.kind, RelationKind::View);
    assert!(
        session
            .table_detail(DATABASE, "missing")
            .await?
            .info
            .is_none()
    );
    Ok(())
}
