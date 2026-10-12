// SPDX-License-Identifier: Apache-2.0
//! Live key enforcement, conversion, concurrent-index and column-position contracts.
use crate::sink::query;
use crate::{Failure, Group, Target, ensure, fixture_name};
use openrows_driver_api::schema::{ConstraintKind, NewIndex, NewKey, SchemaChange};
use openrows_driver_api::structure::{AlterTable, execute_plan};
use openrows_driver_api::{Connection, Feature, catalog};

pub(crate) async fn run(
    target: &Target,
    connection: &mut dyn Connection,
    table: &str,
) -> Result<(), Failure> {
    if !target.driver.capabilities().supports(Feature::KeyEditing) {
        return Ok(());
    }
    let primary = fixture_name(Group::Lifecycle, "pk");
    let unique = fixture_name(Group::Lifecycle, "uq");
    apply(
        target,
        connection,
        table,
        &[
            SchemaChange::AddPrimaryKey(NewKey {
                name: Some(primary),
                columns: vec!["id".into()],
            }),
            SchemaChange::AddUnique(NewKey {
                name: Some(unique),
                columns: vec!["title".into()],
            }),
        ],
    )
    .await?;
    let detail = catalog(connection)?
        .table_detail(&target.schema, table)
        .await?;
    for kind in [ConstraintKind::Primary, ConstraintKind::Unique] {
        ensure(
            detail.constraints.iter().any(|key| key.kind == kind),
            || format!("new {kind:?} constraint not listed"),
        )?;
    }
    enforce(target, connection, table).await?;
    options(target, connection, table).await
}

async fn apply(
    target: &Target,
    connection: &mut dyn Connection,
    table: &str,
    changes: &[SchemaChange],
) -> Result<(), Failure> {
    let detail = catalog(connection)?
        .table_detail(&target.schema, table)
        .await?;
    let plan = target
        .driver
        .alter_sql(
            &AlterTable {
                schema: &target.schema,
                table,
                detail: &detail,
            },
            changes,
        )
        .map_err(|error| Failure::Contract(error.to_string()))?;
    execute_plan(connection, &plan).await?;
    Ok(())
}

async fn enforce(
    target: &Target,
    connection: &mut dyn Connection,
    table: &str,
) -> Result<(), Failure> {
    let relation = target
        .driver
        .dialect()
        .qualified(&target.schema, table)
        .map_err(|e| Failure::Setup(e.to_string()))?;
    query(
        connection,
        &format!("INSERT INTO {relation} (id, title) VALUES (1, 'one')"),
    )
    .await?;
    for values in ["(1, 'two')", "(2, 'one')", "(NULL, 'three')"] {
        let result = query(
            connection,
            &format!("INSERT INTO {relation} (id, title) VALUES {values}"),
        )
        .await;
        ensure(result.is_err(), || format!("key failed to reject {values}"))?;
    }
    let (rows, _) = query(connection, &format!("SELECT COUNT(*) FROM {relation}")).await?;
    ensure(rows.single()?.as_deref() == Some("1"), || {
        "rejected key writes changed rows".into()
    })
}

async fn options(
    target: &Target,
    connection: &mut dyn Connection,
    table: &str,
) -> Result<(), Failure> {
    let caps = target.driver.capabilities();
    if caps.supports(Feature::TypeUsing) {
        conversion(target, connection, table).await?;
    }
    if caps.supports(Feature::ColumnReordering) {
        reorder(target, connection, table).await?;
    }
    if caps.supports(Feature::ConcurrentIndexes) {
        concurrent(target, connection, table).await?;
    }
    Ok(())
}

async fn conversion(
    target: &Target,
    connection: &mut dyn Connection,
    table: &str,
) -> Result<(), Failure> {
    let change = |expression: &str| SchemaChange::ColumnTypeUsing {
        column: "title".into(),
        data_type: "integer".into(),
        expression: expression.into(),
    };
    let failed = apply(target, connection, table, &[change("title::integer")]).await;
    ensure(failed.is_err(), || {
        "an invalid USING conversion succeeded".into()
    })?;
    let relation = target
        .driver
        .dialect()
        .qualified(&target.schema, table)
        .map_err(|e| Failure::Setup(e.to_string()))?;
    let (rows, _) = query(
        connection,
        &format!("SELECT title FROM {relation} WHERE id = 1"),
    )
    .await?;
    ensure(rows.single()?.as_deref() == Some("one"), || {
        "failed USING conversion changed the row".into()
    })?;
    apply(target, connection, table, &[change("length(title)")]).await?;
    let (rows, _) = query(
        connection,
        &format!("SELECT title FROM {relation} WHERE id = 1"),
    )
    .await?;
    ensure(rows.single()?.as_deref() == Some("3"), || {
        "USING did not convert the existing row".into()
    })
}

async fn reorder(
    target: &Target,
    connection: &mut dyn Connection,
    table: &str,
) -> Result<(), Failure> {
    for after in [None, Some("id".into())] {
        let first = after.is_none();
        let mut changes = vec![SchemaChange::ReorderColumn {
            column: "title".into(),
            after,
        }];
        if first {
            changes.push(SchemaChange::ColumnType {
                column: "title".into(),
                data_type: "VARCHAR(50)".into(),
            });
        }
        apply(target, connection, table, &changes).await?;
        let detail = catalog(connection)?
            .table_detail(&target.schema, table)
            .await?;
        let expected = if first {
            ["title", "id"]
        } else {
            ["id", "title"]
        };
        ensure(
            detail.columns.iter().map(|c| c.name.as_str()).eq(expected),
            || "FIRST/AFTER did not move the column".into(),
        )?;
        ensure(
            detail
                .columns
                .iter()
                .any(|c| c.name == "title" && c.data_type.contains("50")),
            || "reordering lost the type change".into(),
        )?;
    }
    Ok(())
}

async fn concurrent(
    target: &Target,
    connection: &mut dyn Connection,
    table: &str,
) -> Result<(), Failure> {
    let index = fixture_name(Group::Lifecycle, "concurrent");
    apply(
        target,
        connection,
        table,
        &[SchemaChange::CreateIndexConcurrently(NewIndex {
            name: Some(index.clone()),
            columns: vec!["title".into()],
            ..NewIndex::default()
        })],
    )
    .await?;
    let detail = catalog(connection)?
        .table_detail(&target.schema, table)
        .await?;
    ensure(detail.indexes.iter().any(|i| i.name == index), || {
        "concurrent index not listed".into()
    })
}
