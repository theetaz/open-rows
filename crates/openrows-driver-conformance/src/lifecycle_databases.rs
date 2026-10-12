// SPDX-License-Identifier: Apache-2.0
//! Database lifecycle executes on the driver's home database outside a transaction.
use crate::{Failure, Group, Target, ensure, fixture_name};
use openrows_driver_api::objects::{ObjectChange, PlanRun};
use openrows_driver_api::{ConnectTrace, Connection, DiscardSink, Feature, catalog};

pub(crate) async fn run(target: &Target) -> Result<(), Failure> {
    if !target
        .driver
        .capabilities()
        .supports(Feature::DatabaseLifecycle)
    {
        return Ok(());
    }
    let mut request = target.request.clone();
    request.database = target.driver.home_database().to_owned();
    let mut connection = target
        .driver
        .connect(&request, &mut ConnectTrace::default())
        .await?;
    let database = fixture_name(Group::Lifecycle, "db");
    let renamed = fixture_name(Group::Lifecycle, "renamed_db");
    let outcome = lifecycle(target, connection.as_mut(), (&database, &renamed)).await;
    if outcome.is_err() {
        for name in [&database, &renamed] {
            let _ = apply(
                target,
                connection.as_mut(),
                &ObjectChange::DropDatabase { name: name.clone() },
            )
            .await;
        }
    }
    let closed = connection.close().await;
    outcome?;
    closed?;
    Ok(())
}

async fn apply(
    target: &Target,
    connection: &mut dyn Connection,
    change: &ObjectChange,
) -> Result<(), Failure> {
    let plan = target
        .driver
        .object_sql(change, None)
        .map_err(|error| Failure::Contract(error.to_string()))?;
    ensure(plan.run == PlanRun::HomeDatabase, || {
        "database DDL must run outside a transaction on the home database".into()
    })?;
    for sql in &plan.statements {
        connection.execute(sql, &mut DiscardSink).await?;
    }
    Ok(())
}

async fn present(
    connection: &mut dyn Connection,
    name: &str,
    expected: bool,
) -> Result<(), Failure> {
    let databases = catalog(connection)?.list_databases().await?;
    let present = databases.iter().any(|d| d.name == name);
    ensure(present == expected, || {
        format!("database {name} present: {present}, expected: {expected}")
    })
}

async fn lifecycle(
    target: &Target,
    connection: &mut dyn Connection,
    (database, renamed): (&str, &str),
) -> Result<(), Failure> {
    apply(
        target,
        connection,
        &ObjectChange::CreateDatabase {
            name: database.into(),
            encoding: None,
            collation: None,
        },
    )
    .await?;
    present(connection, database, true).await?;
    let name = if target
        .driver
        .capabilities()
        .supports(Feature::DatabaseRename)
    {
        apply(
            target,
            connection,
            &ObjectChange::RenameDatabase {
                name: database.into(),
                to: renamed.into(),
            },
        )
        .await?;
        present(connection, database, false).await?;
        present(connection, renamed, true).await?;
        renamed
    } else {
        database
    };
    apply(
        target,
        connection,
        &ObjectChange::DropDatabase { name: name.into() },
    )
    .await?;
    present(connection, name, false).await
}
