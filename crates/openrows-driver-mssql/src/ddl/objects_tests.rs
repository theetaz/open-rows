// SPDX-License-Identifier: Apache-2.0
use openrows_driver_api::objects::RoutineKind;
use openrows_driver_api::schema::{
    ColumnDetail, ConstraintDetail, DefaultValue, ForeignTarget, Identity, NewCheck, NewColumn,
    NewForeignKey, NewIndex, ReferentialAction,
};
use openrows_sql::{StatementKind, classify, split};

use super::*;

const DIALECT: Dialect = Dialect::SqlServer;

fn name(object: &str) -> ObjectName {
    ObjectName {
        schema: "dbo".into(),
        name: object.into(),
    }
}

fn column(name: &str, data_type: &str) -> NewColumn {
    NewColumn {
        name: name.into(),
        data_type: data_type.into(),
        nullable: true,
        ..NewColumn::default()
    }
}

/// What the app runs: single statements that create, change or empty objects.
fn runnable(plan: &ObjectPlan) {
    for sql in &plan.statements {
        assert_eq!(split(sql, DIALECT).len(), 1, "{sql}");
        let kind = classify(sql, DIALECT).kind;
        let insert = kind == StatementKind::Write
            && ["INSERT INTO ", "TRUNCATE TABLE "]
                .iter()
                .any(|head| sql.starts_with(head));
        assert!(kind == StatementKind::Ddl || insert, "{sql}: {kind:?}");
    }
}

fn sql(change: &ObjectChange) -> Result<ObjectPlan, GenerateError> {
    let plan = object_sql(change, None)?;
    runnable(&plan);
    Ok(plan)
}

#[test]
fn tables_are_created_with_keys_indexes_and_comments() -> Result<(), GenerateError> {
    let mut id = column("id", "int");
    id.nullable = false;
    id.identity = Identity::ByDefault;
    let mut title = column("title", "nvarchar(40)");
    title.collation = Some("Latin1_General_BIN".into());
    title.default = DefaultValue::Literal("x".into());
    title.comment = Some("label".into());
    let definition = NewTable {
        columns: vec![id, title, column("owner_id", "int")],
        primary_key: vec!["id".into()],
        indexes: vec![NewIndex {
            columns: vec!["title".into()],
            descending: vec!["title".into()],
            predicate: Some("title IS NOT NULL".into()),
            ..NewIndex::default()
        }],
        foreign_keys: vec![NewForeignKey {
            name: Some("fk_owner".into()),
            columns: vec!["owner_id".into()],
            target: ForeignTarget {
                schema: "dbo".into(),
                table: "owners".into(),
                columns: vec!["id".into()],
            },
            on_delete: ReferentialAction::Cascade,
            on_update: ReferentialAction::Restrict,
        }],
        checks: vec![NewCheck {
            name: None,
            expression: "owner_id > 0".into(),
        }],
        comment: Some("things".into()),
    };
    let plan = sql(&ObjectChange::CreateTable {
        table: name("t"),
        definition,
    })?;
    assert_eq!(
        plan.statements[0],
        "CREATE TABLE [dbo].[t] (\n  [id] int IDENTITY(1, 1) NOT NULL,\n  \
         [title] nvarchar(40) COLLATE Latin1_General_BIN NULL CONSTRAINT [DF_t_title] DEFAULT (N'x'),\n  \
         [owner_id] int NULL,\n  PRIMARY KEY ([id]),\n  \
         CONSTRAINT [fk_owner] FOREIGN KEY ([owner_id]) REFERENCES [dbo].[owners] ([id]) \
         ON DELETE CASCADE ON UPDATE NO ACTION,\n  CHECK (owner_id > 0)\n)"
    );
    assert_eq!(
        plan.statements[1],
        "CREATE INDEX [IX_t_title] ON [dbo].[t] ([title] DESC) WHERE title IS NOT NULL"
    );
    assert_eq!(plan.statements.len(), 4);
    assert!(plan.statements[3].contains("@level2name = N'title'"));
    assert_eq!(plan.run, PlanRun::Transaction);
    Ok(())
}

#[test]
fn tables_rename_duplicate_truncate_and_drop() -> Result<(), GenerateError> {
    let renamed = sql(&ObjectChange::RenameTable {
        table: name("t"),
        name: "u".into(),
    })?;
    assert_eq!(
        renamed.statements,
        ["EXEC sys.sp_rename N'[dbo].[t]', N'u'"]
    );
    let detail = TableDetail {
        columns: vec![
            ColumnDetail {
                name: "id".into(),
                data_type: "int".into(),
                generated: Generated::IdentityAlways,
                ..ColumnDetail::default()
            },
            ColumnDetail {
                name: "twice".into(),
                data_type: "int".into(),
                nullable: true,
                default: Some("([id]*(2))".into()),
                generated: Generated::Stored,
                ..ColumnDetail::default()
            },
        ],
        constraints: vec![ConstraintDetail {
            name: "pk".into(),
            kind: ConstraintKind::Primary,
            definition: String::new(),
            columns: vec!["id".into()],
            references: None,
        }],
        ..TableDetail::default()
    };
    let copy = object_sql(
        &ObjectChange::DuplicateTable {
            table: name("t"),
            name: "t_copy".into(),
            with_data: true,
        },
        Some(&detail),
    )?;
    runnable(&copy);
    assert_eq!(
        copy.statements,
        [
            "CREATE TABLE [dbo].[t_copy] (\n  [id] int NOT NULL,\n  [twice] AS ([id]*(2)),\n  \
             PRIMARY KEY ([id])\n)",
            "INSERT INTO [dbo].[t_copy] ([id]) SELECT [id] FROM [dbo].[t]",
        ]
    );
    let truncate = sql(&ObjectChange::TruncateTable {
        table: name("t"),
        cascade: false,
    })?;
    assert!(truncate.destructive);
    let drop = sql(&ObjectChange::DropTable {
        table: name("t"),
        cascade: false,
    })?;
    assert_eq!(drop.statements, ["DROP TABLE [dbo].[t]"]);
    assert!(
        object_sql(
            &ObjectChange::DropTable {
                table: name("t"),
                cascade: true
            },
            None
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn views_triggers_and_routines_come_from_checked_definitions() -> Result<(), GenerateError> {
    let view = |query: &str, replace: bool| ObjectChange::SaveView {
        view: name("v"),
        query: query.into(),
        materialized: false,
        replace,
    };
    let saved = sql(&view("SELECT id FROM dbo.t;", true))?;
    assert_eq!(
        saved.statements,
        ["CREATE OR ALTER VIEW [dbo].[v] AS\nSELECT id FROM dbo.t"]
    );
    assert!(object_sql(&view("DELETE FROM dbo.t", false), None).is_err());
    let trigger = sql(&ObjectChange::SaveTrigger {
        table: name("t"),
        replaces: Some("tr".into()),
        definition:
            "CREATE TRIGGER dbo.tr ON dbo.t AFTER INSERT AS\nBEGIN\n  SET NOCOUNT ON;\nEND\nGO"
                .into(),
    })?;
    assert_eq!(trigger.statements[0], "DROP TRIGGER IF EXISTS [dbo].[tr]");
    assert!(trigger.statements[1].starts_with("CREATE TRIGGER dbo.tr"));
    let routine = sql(&ObjectChange::SaveRoutine {
        routine: name("p"),
        kind: RoutineKind::Procedure,
        replaces: None,
        definition: "CREATE OR ALTER PROC dbo.p @n int AS SELECT @n; SELECT 2;".into(),
    })?;
    assert_eq!(routine.statements.len(), 1);
    let smuggled = object_sql(
        &ObjectChange::SaveRoutine {
            routine: name("p"),
            kind: RoutineKind::Procedure,
            replaces: None,
            definition: "DROP TABLE dbo.t\nGO\nCREATE PROC dbo.p AS SELECT 1".into(),
        },
        None,
    );
    assert!(smuggled.is_err());
    let dropped = sql(&ObjectChange::DropRoutine {
        routine: name("f"),
        kind: RoutineKind::Function,
        arguments: None,
    })?;
    assert_eq!(dropped.statements, ["DROP FUNCTION [dbo].[f]"]);
    Ok(())
}

#[test]
fn databases_run_on_master_and_unsupported_changes_say_why() -> Result<(), GenerateError> {
    let created = sql(&ObjectChange::CreateDatabase {
        name: "shop".into(),
        encoding: None,
        collation: Some("Latin1_General_100_CI_AS_SC_UTF8".into()),
    })?;
    assert_eq!(
        created.statements,
        ["CREATE DATABASE [shop] COLLATE Latin1_General_100_CI_AS_SC_UTF8"]
    );
    assert_eq!(created.run, PlanRun::HomeDatabase);
    let dropped = sql(&ObjectChange::DropDatabase {
        name: "shop".into(),
    })?;
    assert!(dropped.destructive && dropped.run == PlanRun::HomeDatabase);
    let schema = sql(&ObjectChange::CreateSchema {
        name: "sales".into(),
    })?;
    assert_eq!(schema.statements, ["CREATE SCHEMA [sales]"]);
    for refused in [
        ObjectChange::DropDatabase {
            name: "master".into(),
        },
        ObjectChange::CreateDatabase {
            name: "x".into(),
            encoding: Some("UTF8".into()),
            collation: None,
        },
        ObjectChange::CreateDatabase {
            name: "x".into(),
            encoding: None,
            collation: Some("bad; DROP".into()),
        },
        ObjectChange::RenameDatabase {
            name: "a".into(),
            to: "b".into(),
        },
        ObjectChange::RenameSchema {
            name: "a".into(),
            to: "b".into(),
        },
        ObjectChange::SaveView {
            view: name("m"),
            query: "SELECT 1".into(),
            materialized: true,
            replace: false,
        },
    ] {
        assert!(object_sql(&refused, None).is_err(), "{refused:?}");
    }
    Ok(())
}
