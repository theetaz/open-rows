// SPDX-License-Identifier: Apache-2.0
//! Object lifecycle editors and structure forms against a real server:
//! tables, keys, checks, auto-increment, views, triggers, routines and
//! databases.
use openrows_driver_api::objects::{NewTable, ObjectChange, ObjectName, PlanRun, RoutineKind};
use openrows_driver_api::schema::{
    ConstraintKind, ForeignTarget, Generated, Identity, NewCheck, NewColumn, NewForeignKey,
    ReferentialAction, SchemaChange,
};
use openrows_driver_api::structure::AlterTable;
use openrows_driver_api::{Catalog, Driver, Editor};
use openrows_driver_mysql::{Flavour, MySqlDriver, MySqlSession};

use super::support::{DATABASE, TestResult, mariadb, run, session};

fn driver() -> MySqlDriver {
    MySqlDriver(if mariadb() {
        Flavour::MariaDb
    } else {
        Flavour::MySql
    })
}

fn name(object: &str) -> ObjectName {
    ObjectName {
        schema: DATABASE.into(),
        name: object.into(),
    }
}

/// Generate `change` and run it the way the app does (`MySQL` commits
/// each statement, so both runs execute them in order).
async fn apply(session: &mut MySqlSession, change: &ObjectChange) -> TestResult {
    let source = match change {
        ObjectChange::DuplicateTable { table, .. } => {
            Some(session.table_detail(&table.schema, &table.name).await?)
        }
        _ => None,
    };
    let plan = driver().object_sql(change, source.as_ref())?;
    assert!(matches!(
        plan.run,
        PlanRun::Transaction | PlanRun::HomeDatabase
    ));
    session.commit_ddl(&plan.statements).await?;
    Ok(())
}

fn owners() -> ObjectChange {
    ObjectChange::CreateTable {
        table: name("obj_owners"),
        definition: NewTable {
            columns: vec![
                NewColumn {
                    name: "id".into(),
                    data_type: "bigint unsigned".into(),
                    identity: Identity::AutoIncrement,
                    ..NewColumn::default()
                },
                NewColumn {
                    name: "label".into(),
                    data_type: "varchar(40)".into(),
                    nullable: true,
                    charset: Some("utf8mb4".into()),
                    collation: Some("utf8mb4_bin".into()),
                    comment: Some("Shown".into()),
                    ..NewColumn::default()
                },
                NewColumn {
                    name: "owner_id".into(),
                    data_type: "bigint unsigned".into(),
                    nullable: true,
                    ..NewColumn::default()
                },
            ],
            primary_key: vec!["id".into()],
            comment: Some("Owners".into()),
            ..NewTable::default()
        },
    }
}

async fn drop_fixtures(session: &mut MySqlSession) -> TestResult {
    run(
        session,
        &[
            "DROP VIEW IF EXISTS obj_view",
            "DROP TABLE IF EXISTS obj_owners_copy",
            "DROP TABLE IF EXISTS obj_owners",
            "DROP PROCEDURE IF EXISTS obj_proc",
            "DROP DATABASE IF EXISTS orc_objects_db",
        ],
    )
    .await?;
    Ok(())
}

/// Add a foreign key, a check and a character set, then drop the constraints.
async fn keys(session: &mut MySqlSession) -> TestResult {
    let detail = session.table_detail(DATABASE, "obj_owners").await?;
    assert_eq!(detail.columns[0].generated, Generated::AutoIncrement);
    let changes = [
        SchemaChange::AddForeignKey(NewForeignKey {
            name: Some("obj_owner_fk".into()),
            columns: vec!["owner_id".into()],
            target: ForeignTarget {
                schema: String::new(),
                table: "obj_owners".into(),
                columns: vec!["id".into()],
            },
            on_delete: ReferentialAction::SetNull,
            on_update: ReferentialAction::NoAction,
        }),
        SchemaChange::AddCheck(NewCheck {
            name: Some("obj_label_set".into()),
            expression: "label <> ''".into(),
        }),
        SchemaChange::ColumnCollation {
            column: "label".into(),
            charset: Some("latin1".into()),
            collation: None,
        },
    ];
    let target = AlterTable {
        schema: DATABASE,
        table: "obj_owners",
        detail: &detail,
    };
    session
        .commit_ddl(&driver().alter_sql(&target, &changes)?.statements)
        .await?;
    let detail = session.table_detail(DATABASE, "obj_owners").await?;
    let kinds: Vec<_> = detail.constraints.iter().map(|c| c.kind).collect();
    assert!(kinds.contains(&ConstraintKind::Foreign), "{kinds:?}");
    assert!(kinds.contains(&ConstraintKind::Check), "{kinds:?}");
    let target = AlterTable {
        schema: DATABASE,
        table: "obj_owners",
        detail: &detail,
    };
    let drops = [
        SchemaChange::DropConstraint("obj_owner_fk".into()),
        SchemaChange::DropConstraint("obj_label_set".into()),
    ];
    session
        .commit_ddl(&driver().alter_sql(&target, &drops)?.statements)
        .await?;
    Ok(())
}

#[tokio::test]
#[ignore = "needs MySQL or MariaDB; run with `just test-mysql`"]
async fn tables_keys_and_copies() -> TestResult {
    let mut session = session().await?;
    drop_fixtures(&mut session).await?;
    apply(&mut session, &owners()).await?;
    keys(&mut session).await?;
    run(
        &mut session,
        &["INSERT INTO obj_owners (label) VALUES ('a'), ('b')"],
    )
    .await?;
    apply(
        &mut session,
        &ObjectChange::DuplicateTable {
            table: name("obj_owners"),
            name: "obj_owners_copy".into(),
            with_data: true,
        },
    )
    .await?;
    apply(
        &mut session,
        &ObjectChange::TruncateTable {
            table: name("obj_owners_copy"),
            cascade: false,
        },
    )
    .await?;
    apply(
        &mut session,
        &ObjectChange::DropTable {
            table: name("obj_owners_copy"),
            cascade: false,
        },
    )
    .await?;
    drop_fixtures(&mut session).await
}

#[tokio::test]
#[ignore = "needs MySQL or MariaDB; run with `just test-mysql`"]
async fn views_triggers_routines_and_databases() -> TestResult {
    let mut session = session().await?;
    drop_fixtures(&mut session).await?;
    apply(&mut session, &owners()).await?;
    for replace in [false, true] {
        let view = ObjectChange::SaveView {
            view: name("obj_view"),
            query: "SELECT id, label FROM obj_owners".into(),
            materialized: false,
            replace,
        };
        apply(&mut session, &view).await?;
    }
    for replaces in [None, Some("obj_touch".to_owned())] {
        let trigger = ObjectChange::SaveTrigger {
            table: name("obj_owners"),
            replaces,
            definition: "CREATE TRIGGER obj_touch BEFORE INSERT ON obj_owners FOR EACH ROW \
                         BEGIN SET NEW.label = COALESCE(NEW.label, 'x'); END"
                .into(),
        };
        apply(&mut session, &trigger).await?;
    }
    let detail = session.table_detail(DATABASE, "obj_owners").await?;
    assert_eq!(detail.triggers.len(), 1);
    let procedure = "CREATE PROCEDURE obj_proc() BEGIN SELECT 1; END";
    apply(
        &mut session,
        &ObjectChange::SaveRoutine {
            routine: name("obj_proc"),
            kind: RoutineKind::Procedure,
            replaces: None,
            definition: procedure.into(),
        },
    )
    .await?;
    let definition = session
        .routine_definition((DATABASE, "obj_proc"), RoutineKind::Procedure, None)
        .await?
        .ok_or("definition")?;
    assert!(definition.contains("PROCEDURE `obj_proc`"), "{definition}");
    apply(
        &mut session,
        &ObjectChange::SaveRoutine {
            routine: name("obj_proc"),
            kind: RoutineKind::Procedure,
            replaces: Some(String::new()),
            definition,
        },
    )
    .await?;
    assert!(
        session
            .routine_definition((DATABASE, "obj_missing"), RoutineKind::Function, None)
            .await?
            .is_none()
    );
    apply(
        &mut session,
        &ObjectChange::CreateDatabase {
            name: "orc_objects_db".into(),
            encoding: Some("utf8mb4".into()),
            collation: Some("utf8mb4_bin".into()),
        },
    )
    .await?;
    apply(
        &mut session,
        &ObjectChange::DropDatabase {
            name: "orc_objects_db".into(),
        },
    )
    .await?;
    drop_fixtures(&mut session).await
}
