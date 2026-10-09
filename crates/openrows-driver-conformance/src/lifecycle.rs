// SPDX-License-Identifier: Apache-2.0
//! [`crate::Group::Lifecycle`]: tables, indexes and views created and dropped
//! through generated DDL (`Driver::object_sql` and `Driver::alter_sql`,
//! committed with `Editor::commit_ddl`) appear in and disappear from the
//! catalog; an invalid view query is refused before any SQL runs.

use openrows_driver_api::catalog::ObjectKind;
use openrows_driver_api::objects::{NewTable, ObjectChange, ObjectName, PlanRun};
use openrows_driver_api::schema::{NewColumn, NewIndex, SchemaChange};
use openrows_driver_api::structure::AlterTable;
use openrows_driver_api::{Connection, catalog, editor};

use crate::{Failure, Group, Target, ensure, fixture_name};

pub(crate) async fn run(target: &Target) -> Result<(), Failure> {
    let table = fixture_name(Group::Lifecycle, "t");
    let view = fixture_name(Group::Lifecycle, "v");
    let mut connection = target.connect().await?;
    let outcome = lifecycle(target, connection.as_mut(), (&table, &view)).await;
    if outcome.is_err() {
        // Best effort: leave nothing behind for the next run.
        for change in [drop_view(target, &view), drop_table(target, &table)] {
            let _ = apply(target, connection.as_mut(), &change).await;
        }
    }
    connection.close().await?;
    outcome
}

fn name(target: &Target, name: &str) -> ObjectName {
    ObjectName {
        schema: target.schema.clone(),
        name: name.to_owned(),
    }
}

fn create_table(target: &Target, table: &str) -> ObjectChange {
    let column = |name: &str, data_type: &str, nullable: bool| NewColumn {
        name: name.to_owned(),
        data_type: data_type.to_owned(),
        nullable,
        ..NewColumn::default()
    };
    ObjectChange::CreateTable {
        table: name(target, table),
        definition: NewTable {
            columns: vec![
                column("id", "INTEGER", false),
                column("title", "VARCHAR(40)", true),
            ],
            primary_key: vec!["id".to_owned()],
            ..NewTable::default()
        },
    }
}

fn drop_table(target: &Target, table: &str) -> ObjectChange {
    ObjectChange::DropTable {
        table: name(target, table),
        cascade: false,
    }
}

fn drop_view(target: &Target, view: &str) -> ObjectChange {
    ObjectChange::DropView {
        view: name(target, view),
        materialized: false,
        cascade: false,
    }
}

/// Generate `change` and commit it in one transaction.
async fn apply(
    target: &Target,
    connection: &mut dyn Connection,
    change: &ObjectChange,
) -> Result<(), Failure> {
    let plan = target
        .driver
        .object_sql(change, None)
        .map_err(|error| Failure::Contract(format!("object_sql: {error}")))?;
    ensure(plan.run == PlanRun::Transaction, || {
        format!("{change:?} must run in a transaction")
    })?;
    editor(connection)?.commit_ddl(&plan.statements).await?;
    Ok(())
}

async fn listed(
    target: &Target,
    connection: &mut dyn Connection,
    kind: ObjectKind,
) -> Result<Vec<String>, Failure> {
    let objects = catalog(connection)?
        .list_objects(&target.schema, &[kind])
        .await?;
    Ok(objects.into_iter().map(|object| object.name).collect())
}

async fn lifecycle(
    target: &Target,
    connection: &mut dyn Connection,
    (table, view): (&str, &str),
) -> Result<(), Failure> {
    apply(target, connection, &create_table(target, table)).await?;
    let tables = listed(target, connection, ObjectKind::Table).await?;
    ensure(tables.iter().any(|name| name == table), || {
        format!("created table {table} not listed: {tables:?}")
    })?;
    index(target, connection, table).await?;
    view_lifecycle(target, connection, (table, view)).await?;
    apply(target, connection, &drop_table(target, table)).await?;
    let tables = listed(target, connection, ObjectKind::Table).await?;
    ensure(!tables.iter().any(|name| name == table), || {
        format!("dropped table {table} still listed")
    })
}

/// Create an index through `alter_sql`, then drop it.
async fn index(
    target: &Target,
    connection: &mut dyn Connection,
    table: &str,
) -> Result<(), Failure> {
    let index = fixture_name(Group::Lifecycle, "i");
    for change in [
        SchemaChange::CreateIndex(NewIndex {
            name: Some(index.clone()),
            columns: vec!["title".to_owned()],
            ..NewIndex::default()
        }),
        SchemaChange::DropIndex(index.clone()),
    ] {
        let detail = catalog(connection)?
            .table_detail(&target.schema, table)
            .await?;
        let place = AlterTable {
            schema: &target.schema,
            table,
            detail: &detail,
        };
        let plan = target
            .driver
            .alter_sql(&place, std::slice::from_ref(&change))
            .map_err(|error| Failure::Contract(format!("alter_sql: {error}")))?;
        editor(connection)?.commit_ddl(&plan.statements).await?;
        let after = catalog(connection)?
            .table_detail(&target.schema, table)
            .await?;
        let present = after.indexes.iter().any(|i| i.name == index);
        let created = matches!(change, SchemaChange::CreateIndex(_));
        ensure(present == created, || {
            format!("index {index} present: {present} after {change:?}")
        })?;
    }
    Ok(())
}

async fn view_lifecycle(
    target: &Target,
    connection: &mut dyn Connection,
    (table, view): (&str, &str),
) -> Result<(), Failure> {
    let relation = target
        .driver
        .dialect()
        .qualified(&target.schema, table)
        .map_err(|error| Failure::Setup(error.to_string()))?;
    let save = |query: String| ObjectChange::SaveView {
        view: name(target, view),
        query,
        materialized: false,
        replace: false,
    };
    ensure(
        target
            .driver
            .object_sql(&save(format!("DELETE FROM {relation}")), None)
            .is_err(),
        || "a view over DELETE was not refused".to_owned(),
    )?;
    apply(
        target,
        connection,
        &save(format!("SELECT id, title FROM {relation}")),
    )
    .await?;
    let views = listed(target, connection, ObjectKind::View).await?;
    ensure(views.iter().any(|name| name == view), || {
        format!("created view {view} not listed: {views:?}")
    })?;
    apply(target, connection, &drop_view(target, view)).await?;
    let views = listed(target, connection, ObjectKind::View).await?;
    ensure(!views.iter().any(|name| name == view), || {
        format!("dropped view {view} still listed")
    })
}
