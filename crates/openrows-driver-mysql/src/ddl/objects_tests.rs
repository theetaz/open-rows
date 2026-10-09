// SPDX-License-Identifier: Apache-2.0
use super::*;
use openrows_driver_api::objects::RoutineKind;
use openrows_driver_api::schema::{
    ColumnDetail, DefaultValue, ForeignTarget, Identity, NewCheck, NewColumn, NewForeignKey,
    NewIndex, ReferentialAction,
};

fn name(schema: &str, name: &str) -> ObjectName {
    ObjectName {
        schema: schema.into(),
        name: name.into(),
    }
}

fn sql(change: &ObjectChange) -> Result<Vec<String>, GenerateError> {
    object_sql(change, None).map(|plan| plan.statements)
}

#[test]
fn new_tables_are_one_statement() -> Result<(), GenerateError> {
    let definition = NewTable {
        columns: vec![
            NewColumn {
                name: "id".into(),
                data_type: "bigint unsigned".into(),
                identity: Identity::AutoIncrement,
                ..NewColumn::default()
            },
            NewColumn {
                name: "title".into(),
                data_type: "varchar(80)".into(),
                nullable: true,
                default: DefaultValue::Literal("untitled".into()),
                charset: Some("utf8mb4".into()),
                collation: Some("utf8mb4_bin".into()),
                comment: Some("Shown".into()),
                ..NewColumn::default()
            },
            NewColumn {
                name: "owner_id".into(),
                data_type: "int".into(),
                ..NewColumn::default()
            },
        ],
        primary_key: vec!["id".into()],
        indexes: vec![NewIndex {
            name: Some("by_title".into()),
            columns: vec!["title".into()],
            descending: vec!["title".into()],
            ..NewIndex::default()
        }],
        foreign_keys: vec![NewForeignKey {
            name: None,
            columns: vec!["owner_id".into()],
            target: ForeignTarget {
                schema: String::new(),
                table: "owners".into(),
                columns: vec!["id".into()],
            },
            on_delete: ReferentialAction::Restrict,
            on_update: ReferentialAction::NoAction,
        }],
        checks: vec![NewCheck {
            name: Some("positive".into()),
            expression: "owner_id > 0".into(),
        }],
        comment: Some("Notes".into()),
    };
    assert_eq!(
        sql(&ObjectChange::CreateTable {
            table: name("shop", "notes"),
            definition,
        })?,
        ["CREATE TABLE `shop`.`notes` (\n  \
          `id` bigint unsigned NOT NULL AUTO_INCREMENT,\n  \
          `title` varchar(80) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin DEFAULT 'untitled' COMMENT 'Shown',\n  \
          `owner_id` int NOT NULL,\n  \
          PRIMARY KEY (`id`),\n  \
          INDEX `by_title` (`title` DESC) USING BTREE,\n  \
          FOREIGN KEY (`owner_id`) REFERENCES `shop`.`owners` (`id`) ON DELETE RESTRICT,\n  \
          CONSTRAINT `positive` CHECK (owner_id > 0)\n) COMMENT = 'Notes'"]
    );
    Ok(())
}

#[test]
fn table_lifecycle_statements() -> Result<(), GenerateError> {
    let table = name("shop", "orders");
    assert_eq!(
        sql(&ObjectChange::RenameTable {
            table: table.clone(),
            name: "sales".into()
        })?,
        ["RENAME TABLE `shop`.`orders` TO `shop`.`sales`"]
    );
    let detail = TableDetail {
        columns: vec![
            ColumnDetail {
                name: "id".into(),
                ..ColumnDetail::default()
            },
            ColumnDetail {
                name: "total".into(),
                generated: Generated::Stored,
                ..ColumnDetail::default()
            },
        ],
        ..TableDetail::default()
    };
    let copy = object_sql(
        &ObjectChange::DuplicateTable {
            table: table.clone(),
            name: "orders_copy".into(),
            with_data: true,
        },
        Some(&detail),
    )?;
    assert_eq!(
        copy.statements,
        [
            "CREATE TABLE `shop`.`orders_copy` LIKE `shop`.`orders`",
            "INSERT INTO `shop`.`orders_copy` (`id`) SELECT `id` FROM `shop`.`orders`",
        ]
    );
    assert!(
        sql(&ObjectChange::TruncateTable {
            table: table.clone(),
            cascade: true
        })
        .is_err()
    );
    assert_eq!(
        sql(&ObjectChange::DropTable {
            table,
            cascade: false
        })?,
        ["DROP TABLE `shop`.`orders`"]
    );
    Ok(())
}

#[test]
fn definitions_replace_by_dropping_first() -> Result<(), GenerateError> {
    let trigger = object_sql(
        &ObjectChange::SaveTrigger {
            table: name("shop", "orders"),
            replaces: Some("touch".into()),
            definition: "CREATE TRIGGER touch BEFORE UPDATE ON orders FOR EACH ROW BEGIN SET NEW.at = NOW(); END".into(),
        },
        None,
    )?;
    assert_eq!(
        trigger.statements[0],
        "DROP TRIGGER IF EXISTS `shop`.`touch`"
    );
    assert_eq!(trigger.warnings, [NOT_ATOMIC]);
    let routine = sql(&ObjectChange::SaveRoutine {
        routine: name("shop", "total"),
        kind: RoutineKind::Procedure,
        replaces: Some(String::new()),
        definition: "CREATE PROCEDURE total() BEGIN SELECT 1; END".into(),
    })?;
    assert_eq!(routine[0], "DROP PROCEDURE IF EXISTS `shop`.`total`");
    let view = sql(&ObjectChange::SaveView {
        view: name("shop", "v"),
        query: "SELECT 1".into(),
        materialized: false,
        replace: true,
    })?;
    assert_eq!(view, ["CREATE OR REPLACE VIEW `shop`.`v` AS\nSELECT 1"]);
    Ok(())
}

#[test]
fn databases_but_not_schemas_or_renames() -> Result<(), GenerateError> {
    let create = object_sql(
        &ObjectChange::CreateDatabase {
            name: "shop".into(),
            encoding: Some("utf8mb4".into()),
            collation: Some("utf8mb4_0900_ai_ci".into()),
        },
        None,
    )?;
    assert_eq!(
        create.statements,
        ["CREATE DATABASE `shop` CHARACTER SET utf8mb4 COLLATE utf8mb4_0900_ai_ci"]
    );
    assert_eq!(create.run, PlanRun::HomeDatabase);
    assert!(
        sql(&ObjectChange::CreateDatabase {
            name: "x".into(),
            encoding: Some("utf8; DROP".into()),
            collation: None
        })
        .is_err()
    );
    assert!(
        sql(&ObjectChange::DropDatabase {
            name: "mysql".into()
        })
        .is_err()
    );
    assert!(
        sql(&ObjectChange::RenameDatabase {
            name: "a".into(),
            to: "b".into()
        })
        .is_err()
    );
    assert!(sql(&ObjectChange::CreateSchema { name: "s".into() }).is_err());
    Ok(())
}
