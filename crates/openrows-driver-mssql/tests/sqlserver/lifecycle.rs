// SPDX-License-Identifier: Apache-2.0
//! Object lifecycle plans (`Driver::object_sql`) run against the server:
//! table duplication with rows, triggers, routines and their stored
//! definition, databases on `master`, and foreign keys and checks added
//! through structure changes.
use arrow_array::cast::AsArray;
use arrow_array::types::Int32Type;
use openrows_driver_api::catalog::ObjectKind;
use openrows_driver_api::objects::{ObjectChange, ObjectName, PlanRun, RoutineKind};
use openrows_driver_api::schema::{ForeignTarget, NewCheck, NewForeignKey, SchemaChange};
use openrows_driver_api::structure::AlterTable;
use openrows_driver_api::{Catalog, Driver, Editor, Session};
use openrows_driver_mssql::{SqlServerDriver, SqlServerSession};

use super::support::{Collect, TestResult, code, fixture, run, session};

fn name(object: &str) -> ObjectName {
    ObjectName {
        schema: "dbo".into(),
        name: object.into(),
    }
}

async fn apply(session: &mut SqlServerSession, change: &ObjectChange) -> TestResult {
    let source = match change {
        ObjectChange::DuplicateTable { table, .. } => {
            Some(session.table_detail(&table.schema, &table.name).await?)
        }
        _ => None,
    };
    let plan = SqlServerDriver.object_sql(change, source.as_ref())?;
    match plan.run {
        PlanRun::Transaction => session.commit_ddl(&plan.statements).await?,
        PlanRun::HomeDatabase => {
            let statements: Vec<&str> = plan.statements.iter().map(String::as_str).collect();
            run(session, &statements).await?;
        }
    }
    Ok(())
}

#[tokio::test]
#[ignore = "needs SQL Server; run with `just test-mssql`"]
async fn tables_triggers_and_routines_round_trip() -> TestResult {
    let mut session = session().await?;
    let (table, copy) = (fixture("life"), fixture("life_copy"));
    run(
        &mut session,
        &[
            &format!(
                "CREATE TABLE dbo.{table} (id int IDENTITY PRIMARY KEY, n int, twice AS n * 2)"
            ),
            &format!("INSERT INTO dbo.{table} (n) VALUES (1), (2)"),
        ],
    )
    .await?;
    let duplicate = ObjectChange::DuplicateTable {
        table: name(&table),
        name: copy.clone(),
        with_data: true,
    };
    apply(&mut session, &duplicate).await?;
    let mut sink = Collect::default();
    session
        .execute(&format!("SELECT SUM(twice) FROM dbo.{copy}"), &mut sink)
        .await?;
    assert_eq!(
        sink.batches[0]
            .column(0)
            .as_primitive::<Int32Type>()
            .value(0),
        6
    );
    let trigger = format!(
        "CREATE TRIGGER dbo.{table}_tr ON dbo.{table} AFTER INSERT AS\nBEGIN\n  SET NOCOUNT ON;\nEND"
    );
    apply(
        &mut session,
        &ObjectChange::SaveTrigger {
            table: name(&table),
            replaces: None,
            definition: trigger,
        },
    )
    .await?;
    assert_eq!(session.table_detail("dbo", &table).await?.triggers.len(), 1);
    routines(&mut session, &table).await?;
    constraints(&mut session, (&table, &copy)).await?;
    for dropped in [&copy, &table] {
        let change = ObjectChange::DropTable {
            table: name(dropped),
            cascade: false,
        };
        apply(&mut session, &change).await?;
    }
    Ok(())
}

async fn routines(session: &mut SqlServerSession, table: &str) -> TestResult {
    let procedure = format!("{table}_p");
    let definition =
        format!("CREATE OR ALTER PROC dbo.{procedure} @n int AS\nSELECT @n * 2;\nSELECT 3;");
    apply(
        session,
        &ObjectChange::SaveRoutine {
            routine: name(&procedure),
            kind: RoutineKind::Procedure,
            replaces: Some(String::new()),
            definition: definition.clone(),
        },
    )
    .await?;
    let stored = session
        .routine_definition(("dbo", &procedure), RoutineKind::Procedure, None)
        .await?;
    // The server stores `CREATE OR ALTER` as `CREATE`; the body is kept as written.
    let body = format!("PROC dbo.{procedure} @n int AS\nSELECT @n * 2;\nSELECT 3;");
    assert!(stored.is_some_and(|text| text.starts_with("CREATE") && text.ends_with(&body)));
    let listed = session
        .list_objects("dbo", &[ObjectKind::Procedure])
        .await?;
    assert!(listed.iter().any(|object| object.name == procedure));
    let drop = ObjectChange::DropRoutine {
        routine: name(&procedure),
        kind: RoutineKind::Procedure,
        arguments: None,
    };
    apply(session, &drop).await
}

async fn constraints(session: &mut SqlServerSession, (table, copy): (&str, &str)) -> TestResult {
    let detail = session.table_detail("dbo", copy).await?;
    let changes = [
        SchemaChange::AddForeignKey(NewForeignKey {
            name: Some(format!("fk_{copy}")),
            columns: vec!["id".into()],
            target: ForeignTarget {
                schema: "dbo".into(),
                table: table.into(),
                columns: vec!["id".into()],
            },
            ..NewForeignKey::default()
        }),
        SchemaChange::AddCheck(NewCheck {
            name: Some(format!("ck_{copy}")),
            expression: "n > 0".into(),
        }),
    ];
    let target = AlterTable {
        schema: "dbo",
        table: copy,
        detail: &detail,
    };
    let plan = SqlServerDriver.alter_sql(&target, &changes)?;
    session.commit_ddl(&plan.statements).await?;
    let insert = format!("INSERT INTO dbo.{copy} (id, n) VALUES (99, 1)");
    let failed = session.execute(&insert, &mut Collect::default()).await;
    assert_eq!(code(failed), "23000");
    let after = session.table_detail("dbo", copy).await?;
    let target = AlterTable {
        schema: "dbo",
        table: copy,
        detail: &after,
    };
    let drop = [SchemaChange::DropConstraint(format!("fk_{copy}"))];
    let plan = SqlServerDriver.alter_sql(&target, &drop)?;
    session.commit_ddl(&plan.statements).await?;
    Ok(())
}

#[tokio::test]
#[ignore = "needs SQL Server; run with `just test-mssql`"]
async fn databases_are_created_and_dropped_on_master() -> TestResult {
    let mut session = session().await?;
    let database = fixture("db");
    apply(
        &mut session,
        &ObjectChange::CreateDatabase {
            name: database.clone(),
            encoding: None,
            collation: Some("Latin1_General_100_CI_AS".into()),
        },
    )
    .await?;
    assert!(
        session
            .list_databases()
            .await?
            .iter()
            .any(|db| db.name == database)
    );
    apply(
        &mut session,
        &ObjectChange::DropDatabase {
            name: database.clone(),
        },
    )
    .await?;
    assert!(
        !session
            .list_databases()
            .await?
            .iter()
            .any(|db| db.name == database)
    );
    Ok(())
}
