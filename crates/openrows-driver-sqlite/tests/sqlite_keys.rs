// SPDX-License-Identifier: Apache-2.0
//! Key additions preserve data and roll back failed rebuilds.
mod support;
use openrows_driver_api::schema::{NewKey, SchemaChange};
use openrows_driver_api::structure::AlterTable;
use openrows_driver_api::{Catalog, Driver, Editor, Session};
use openrows_driver_sqlite::{SqliteDriver, SqliteSession};
use support::{Collect, TestResult, database, run};

async fn add(session: &mut SqliteSession, change: SchemaChange) -> TestResult {
    let detail = session.table_detail("main", "items").await?;
    let plan = SqliteDriver.alter_sql(
        &AlterTable {
            schema: "main",
            table: "items",
            detail: &detail,
        },
        &[change],
    )?;
    session.commit_ddl(&plan.statements).await?;
    Ok(())
}

fn key(column: &str) -> NewKey {
    NewKey {
        name: Some("items_key".into()),
        columns: vec![column.into()],
    }
}

#[tokio::test]
async fn primary_key_makes_nullable_columns_not_null_and_keeps_rows() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut session = database(
        &dir,
        "CREATE TABLE items (id TEXT, note TEXT); INSERT INTO items VALUES ('one', 'keep')",
    )
    .await?;
    add(&mut session, SchemaChange::AddPrimaryKey(key("id"))).await?;
    let detail = session.table_detail("main", "items").await?;
    assert!(!detail.columns[0].nullable);
    assert!(
        run(&mut session, "INSERT INTO items VALUES (NULL, 'null')")
            .await
            .is_err()
    );
    assert!(
        run(
            &mut session,
            "INSERT INTO items VALUES ('one', 'duplicate')"
        )
        .await
        .is_err()
    );
    let mut rows = Collect::default();
    session
        .execute(
            "SELECT * FROM items WHERE id = 'one' AND note = 'keep'",
            &mut rows,
        )
        .await?;
    assert_eq!(rows.rows(), 1);
    Box::new(session).close().await?;
    Ok(())
}

#[tokio::test]
async fn duplicate_unique_values_or_null_primary_values_roll_back_everything() -> TestResult {
    for (values, primary) in [("('one'), ('one')", false), ("('one'), (NULL)", true)] {
        let dir = tempfile::tempdir()?;
        let mut session = database(
            &dir,
            &format!("CREATE TABLE items (id TEXT); INSERT INTO items VALUES {values}"),
        )
        .await?;
        let before = session.table_detail("main", "items").await?;
        let change = if primary {
            SchemaChange::AddPrimaryKey(key("id"))
        } else {
            SchemaChange::AddUnique(key("id"))
        };
        assert!(add(&mut session, change).await.is_err());
        let after = session.table_detail("main", "items").await?;
        assert_eq!(before.columns, after.columns);
        assert_eq!(before.constraints, after.constraints);
        let mut rows = Collect::default();
        session.execute("SELECT * FROM items", &mut rows).await?;
        assert_eq!(rows.rows(), 2);
        assert!(
            session
                .table_detail("main", "openrows_rebuild_items")
                .await?
                .info
                .is_none()
        );
        Box::new(session).close().await?;
    }
    Ok(())
}

#[tokio::test]
async fn null_integer_key_is_refused_instead_of_assigning_a_new_row_id() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut session = database(
        &dir,
        "CREATE TABLE items (id INTEGER); INSERT INTO items VALUES (NULL), (7)",
    )
    .await?;
    assert!(
        add(&mut session, SchemaChange::AddPrimaryKey(key("id")))
            .await
            .is_err()
    );
    let mut rows = Collect::default();
    session
        .execute("SELECT * FROM items WHERE id IS NULL", &mut rows)
        .await?;
    assert_eq!(rows.rows(), 1);
    Box::new(session).close().await?;
    Ok(())
}

#[tokio::test]
async fn primary_key_can_target_a_column_renamed_in_the_same_review() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut session = database(
        &dir,
        "CREATE TABLE items (id TEXT); INSERT INTO items VALUES ('one')",
    )
    .await?;
    let detail = session.table_detail("main", "items").await?;
    let changes = [
        SchemaChange::RenameColumn {
            column: "id".into(),
            name: "key_id".into(),
        },
        SchemaChange::AddPrimaryKey(key("key_id")),
    ];
    let plan = SqliteDriver.alter_sql(
        &AlterTable {
            schema: "main",
            table: "items",
            detail: &detail,
        },
        &changes,
    )?;
    session.commit_ddl(&plan.statements).await?;
    let detail = session.table_detail("main", "items").await?;
    assert_eq!(detail.columns[0].name, "key_id");
    assert!(!detail.columns[0].nullable);
    assert_eq!(detail.constraints[0].columns, ["key_id"]);
    let mut rows = Collect::default();
    session
        .execute("SELECT key_id FROM items WHERE key_id = 'one'", &mut rows)
        .await?;
    assert_eq!(rows.rows(), 1);
    Box::new(session).close().await?;
    Ok(())
}

#[tokio::test]
async fn newly_added_integer_primary_key_cannot_silently_number_existing_rows() -> TestResult {
    use openrows_driver_api::schema::NewColumn;
    let dir = tempfile::tempdir()?;
    let mut session = database(
        &dir,
        "CREATE TABLE items (note TEXT); INSERT INTO items VALUES ('keep')",
    )
    .await?;
    let detail = session.table_detail("main", "items").await?;
    let changes = [
        SchemaChange::AddColumn(NewColumn {
            name: "id".into(),
            data_type: "INTEGER".into(),
            ..NewColumn::default()
        }),
        SchemaChange::AddPrimaryKey(key("id")),
    ];
    let result = SqliteDriver.alter_sql(
        &AlterTable {
            schema: "main",
            table: "items",
            detail: &detail,
        },
        &changes,
    );
    assert!(
        matches!(result, Err(openrows_driver_api::GenerateError(message)) if message.contains("separate review"))
    );
    let after = session.table_detail("main", "items").await?;
    assert_eq!(detail.columns, after.columns);
    let mut rows = Collect::default();
    session
        .execute("SELECT note FROM items WHERE note = 'keep'", &mut rows)
        .await?;
    assert_eq!(rows.rows(), 1);
    Box::new(session).close().await?;
    Ok(())
}
