// SPDX-License-Identifier: Apache-2.0
//! Object lifecycle editors and structure forms against a real server:
//! create, duplicate, truncate and drop tables; keys, checks, identity and
//! collation changes; views, materialized views, triggers, routines,
//! schemas and databases.
use crate::support::{Collect, TestResult, connect, run};
use openrows_driver_api::objects::{NewTable, ObjectChange, ObjectName, PlanRun, RoutineKind};
use openrows_driver_api::schema::{
    ConstraintKind, ForeignTarget, Generated, Identity, NewCheck, NewColumn, NewForeignKey,
    ReferentialAction, SchemaChange,
};
use openrows_driver_postgres::PgSession;
use openrows_driver_postgres::ddl::{AlterTable, alter_sql, object_sql};

fn name(table: &str) -> ObjectName {
    ObjectName {
        schema: "t08".into(),
        name: table.into(),
    }
}

/// Generate `change` and run it the way the app does.
async fn apply(session: &mut PgSession, change: &ObjectChange) -> TestResult {
    let source = match change {
        ObjectChange::DuplicateTable { table, .. } => {
            Some(session.table_detail(&table.schema, &table.name).await?)
        }
        _ => None,
    };
    let plan = object_sql(change, source.as_ref())?;
    match plan.run {
        PlanRun::Transaction => session.commit_ddl(&plan.statements).await?,
        PlanRun::HomeDatabase => {
            for statement in &plan.statements {
                let (done, _) = run(session, statement, Collect::default()).await;
                done?;
            }
        }
    }
    Ok(())
}

async fn sql(session: &mut PgSession, statement: &str) -> TestResult {
    let (done, _) = run(session, statement, Collect::default()).await;
    done?;
    Ok(())
}

fn notes() -> ObjectChange {
    let column = |name: &str, data_type: &str| NewColumn {
        name: name.into(),
        data_type: data_type.into(),
        nullable: true,
        ..NewColumn::default()
    };
    ObjectChange::CreateTable {
        table: name("notes"),
        definition: NewTable {
            columns: vec![
                NewColumn {
                    nullable: false,
                    identity: Identity::Always,
                    ..column("id", "bigint")
                },
                column("title", "varchar(40)"),
                column("parent_id", "bigint"),
            ],
            primary_key: vec!["id".into()],
            comment: Some("Notes".into()),
            ..NewTable::default()
        },
    }
}

#[tokio::test]
#[ignore = "needs PostgreSQL: run `just test-pg`"]
async fn tables_are_created_changed_duplicated_and_dropped() -> TestResult {
    let mut session = connect().await?;
    sql(&mut session, "DROP SCHEMA IF EXISTS t08 CASCADE").await?;
    apply(
        &mut session,
        &ObjectChange::CreateSchema { name: "t08".into() },
    )
    .await?;
    apply(&mut session, &notes()).await?;
    keys_and_identity(&mut session).await?;
    copies(&mut session).await?;
    sql(&mut session, "DROP SCHEMA t08 CASCADE").await
}

/// Foreign key, check, collation and identity changes of `t08.notes`.
async fn keys_and_identity(session: &mut PgSession) -> TestResult {
    let detail = session.table_detail("t08", "notes").await?;
    assert_eq!(detail.columns[0].generated, Generated::IdentityAlways);
    let changes = [
        SchemaChange::AddForeignKey(NewForeignKey {
            name: Some("notes_parent_fk".into()),
            columns: vec!["parent_id".into()],
            target: ForeignTarget {
                schema: String::new(),
                table: "notes".into(),
                columns: vec!["id".into()],
            },
            on_delete: ReferentialAction::Cascade,
            on_update: ReferentialAction::NoAction,
        }),
        SchemaChange::AddCheck(NewCheck {
            name: Some("title_set".into()),
            expression: "title <> ''".into(),
        }),
        SchemaChange::ColumnCollation {
            column: "title".into(),
            charset: None,
            collation: Some("C".into()),
        },
        SchemaChange::ColumnIdentity {
            column: "id".into(),
            identity: Identity::ByDefault,
        },
    ];
    let target = AlterTable {
        schema: "t08",
        table: "notes",
        detail: &detail,
    };
    session
        .commit_ddl(&alter_sql(&target, &changes)?.statements)
        .await?;
    let detail = session.table_detail("t08", "notes").await?;
    assert_eq!(detail.columns[0].generated, Generated::IdentityByDefault);
    let kinds: Vec<_> = detail.constraints.iter().map(|c| c.kind).collect();
    assert!(kinds.contains(&ConstraintKind::Foreign) && kinds.contains(&ConstraintKind::Check));
    Ok(())
}

/// Duplicate `t08.notes` with its rows, then truncate, rename and drop the copy.
async fn copies(session: &mut PgSession) -> TestResult {
    sql(session, "INSERT INTO t08.notes (title) VALUES ('a'), ('b')").await?;
    apply(
        session,
        &ObjectChange::DuplicateTable {
            table: name("notes"),
            name: "notes_copy".into(),
            with_data: true,
        },
    )
    .await?;
    let copy = session.table_detail("t08", "notes_copy").await?;
    assert_eq!(copy.columns.len(), 3);
    apply(
        session,
        &ObjectChange::TruncateTable {
            table: name("notes_copy"),
            cascade: false,
        },
    )
    .await?;
    apply(
        session,
        &ObjectChange::RenameTable {
            table: name("notes_copy"),
            name: "notes_old".into(),
        },
    )
    .await?;
    apply(
        session,
        &ObjectChange::DropTable {
            table: name("notes_old"),
            cascade: false,
        },
    )
    .await?;
    assert!(
        session
            .table_detail("t08", "notes_old")
            .await?
            .info
            .is_none()
    );
    Ok(())
}

#[tokio::test]
#[ignore = "needs PostgreSQL: run `just test-pg`"]
async fn views_triggers_routines_schemas_and_databases() -> TestResult {
    let mut session = connect().await?;
    sql(&mut session, "DROP SCHEMA IF EXISTS t09 CASCADE").await?;
    sql(&mut session, "DROP SCHEMA IF EXISTS t09_renamed CASCADE").await?;
    sql(&mut session, "CREATE SCHEMA t09").await?;
    sql(
        &mut session,
        "CREATE TABLE t09.items (id int PRIMARY KEY, at timestamptz)",
    )
    .await?;
    views(&mut session).await?;
    trigger_and_routine(&mut session).await?;
    apply(
        &mut session,
        &ObjectChange::RenameSchema {
            name: "t09".into(),
            to: "t09_renamed".into(),
        },
    )
    .await?;
    apply(
        &mut session,
        &ObjectChange::DropSchema {
            name: "t09_renamed".into(),
            cascade: true,
        },
    )
    .await?;
    databases(&mut session).await
}

/// `t09` names.
fn in_schema(table: &str) -> ObjectName {
    ObjectName {
        schema: "t09".into(),
        name: table.into(),
    }
}

/// Create and replace a view and a materialized view over `t09.items`.
async fn views(session: &mut PgSession) -> TestResult {
    for (materialized, replace) in [(true, false), (true, true), (false, false), (false, true)] {
        let view = if materialized { "items_mv" } else { "items_v" };
        let change = ObjectChange::SaveView {
            view: in_schema(view),
            query: "SELECT id FROM t09.items;".into(),
            materialized,
            replace,
        };
        apply(session, &change).await?;
    }
    let view = session.table_detail("t09", "items_v").await?;
    assert!(view.info.and_then(|info| info.view_definition).is_some());
    Ok(())
}

/// A trigger with its function, the function's definition saved back, then both dropped.
async fn trigger_and_routine(session: &mut PgSession) -> TestResult {
    let trigger = ObjectChange::SaveTrigger {
        table: in_schema("items"),
        replaces: None,
        definition: "CREATE FUNCTION t09.stamp() RETURNS trigger LANGUAGE plpgsql AS $$ \
                     BEGIN NEW.at := now(); RETURN NEW; END $$;\n\
                     CREATE TRIGGER items_stamp BEFORE INSERT ON t09.items \
                     FOR EACH ROW EXECUTE FUNCTION t09.stamp();"
            .into(),
    };
    apply(session, &trigger).await?;
    sql(session, "INSERT INTO t09.items (id) VALUES (1)").await?;
    let detail = session.table_detail("t09", "items").await?;
    assert_eq!(detail.triggers.len(), 1);
    let definition = session
        .routine_definition(("t09", "stamp"), RoutineKind::Function, Some(""))
        .await?
        .ok_or("definition")?;
    assert!(
        definition.starts_with("CREATE OR REPLACE FUNCTION t09.stamp()"),
        "{definition}"
    );
    apply(
        session,
        &ObjectChange::SaveRoutine {
            routine: in_schema("stamp"),
            kind: RoutineKind::Function,
            replaces: Some(String::new()),
            definition,
        },
    )
    .await?;
    apply(
        session,
        &ObjectChange::DropTrigger {
            table: in_schema("items"),
            name: "items_stamp".into(),
        },
    )
    .await?;
    apply(
        session,
        &ObjectChange::DropRoutine {
            routine: in_schema("stamp"),
            kind: RoutineKind::Function,
            arguments: Some(String::new()),
        },
    )
    .await?;
    Ok(())
}

/// Create, rename and drop a database outside a transaction.
async fn databases(session: &mut PgSession) -> TestResult {
    sql(session, "DROP DATABASE IF EXISTS orc_objects_db").await?;
    sql(session, "DROP DATABASE IF EXISTS orc_objects_db2").await?;
    apply(
        session,
        &ObjectChange::CreateDatabase {
            name: "orc_objects_db".into(),
            encoding: Some("UTF8".into()),
            collation: Some("C".into()),
        },
    )
    .await?;
    apply(
        session,
        &ObjectChange::RenameDatabase {
            name: "orc_objects_db".into(),
            to: "orc_objects_db2".into(),
        },
    )
    .await?;
    let names: Vec<String> = session
        .list_databases()
        .await?
        .into_iter()
        .map(|database| database.name)
        .collect();
    assert!(names.contains(&"orc_objects_db2".to_owned()), "{names:?}");
    apply(
        session,
        &ObjectChange::DropDatabase {
            name: "orc_objects_db2".into(),
        },
    )
    .await
}
