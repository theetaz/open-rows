// SPDX-License-Identifier: Apache-2.0
//! Primary-key plans never convert source NULLs in strict or non-strict sessions.
use super::support::{Collect, DATABASE, TestResult, mariadb, run, session};
use openrows_driver_api::schema::{ConstraintKind, NewKey, SchemaChange};
use openrows_driver_api::structure::{AlterTable, execute_plan};
use openrows_driver_api::{Catalog, Driver, Session};
use openrows_driver_mysql::{Flavour, MySqlDriver, MySqlSession};

async fn refuses_null_key(connection: &mut MySqlSession) -> TestResult {
    let table = format!("key_null_{}", std::process::id());
    run(
        connection,
        &[
            &format!("CREATE TABLE `{table}` (id BIGINT)"),
            &format!("INSERT INTO `{table}` VALUES (NULL), (7)"),
        ],
    )
    .await?;
    let before = connection.table_detail(DATABASE, &table).await?;
    let driver = MySqlDriver(if mariadb() {
        Flavour::MariaDb
    } else {
        Flavour::MySql
    });
    let change = SchemaChange::AddPrimaryKey(NewKey {
        name: None,
        columns: vec!["id".into()],
    });
    let plan = driver.alter_sql(
        &AlterTable {
            schema: DATABASE,
            table: &table,
            detail: &before,
        },
        &[change],
    )?;
    let result = execute_plan(connection, &plan).await;
    assert!(result.is_err());
    let after = connection.table_detail(DATABASE, &table).await?;
    assert_eq!(before.columns, after.columns);
    assert!(
        !after
            .constraints
            .iter()
            .any(|key| key.kind == ConstraintKind::Primary)
    );
    let mut rows = Collect::default();
    connection
        .execute(
            &format!("SELECT id FROM `{table}` WHERE id IS NULL"),
            &mut rows,
        )
        .await?;
    assert_eq!(rows.rows(), 1);
    run(connection, &[&format!("DROP TABLE `{table}`")]).await?;
    Ok(())
}

#[tokio::test]
#[ignore = "needs MySQL or MariaDB; run with just test-mysql"]
async fn primary_key_refuses_nulls_and_non_strict_mode_without_changing_rows() -> TestResult {
    let mut connection = session().await?;
    refuses_null_key(&mut connection).await?;
    run(&mut connection, &["SET SESSION sql_mode = ''"]).await?;
    refuses_null_key(&mut connection).await?;
    Box::new(connection).close().await?;
    Ok(())
}
