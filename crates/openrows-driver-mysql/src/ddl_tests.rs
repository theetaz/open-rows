// SPDX-License-Identifier: Apache-2.0
use super::*;
use openrows_driver_api::schema::{
    ColumnDetail, IndexDetail, RelationKind, TableDetail, TableInfo,
};

const SHOW_CREATE: &str = "CREATE TABLE `orders` (
  `id` bigint unsigned NOT NULL AUTO_INCREMENT,
  `note` varchar(40) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin DEFAULT 'n/a' COMMENT 'free text',
  `updated` timestamp NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  PRIMARY KEY (`id`),
  UNIQUE KEY `note_key` (`note`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
";

fn detail() -> TableDetail {
    let column = |name: &str| ColumnDetail {
        name: name.into(),
        nullable: true,
        ..ColumnDetail::default()
    };
    let index = |name: &str, primary: bool| IndexDetail {
        name: name.into(),
        unique: true,
        primary,
        constraint: true,
        ..IndexDetail::default()
    };
    TableDetail {
        columns: vec![column("id"), column("note"), column("updated")],
        indexes: vec![index("PRIMARY", true), index("note_key", false)],
        info: Some(TableInfo {
            kind: RelationKind::Table,
            definition: Some(SHOW_CREATE.into()),
            ..TableInfo::default()
        }),
        ..TableDetail::default()
    }
}

fn plan(changes: &[SchemaChange]) -> Result<AlterPlan, AlterError> {
    let detail = detail();
    alter_sql(
        &AlterTable {
            schema: "shop",
            table: "orders",
            detail: &detail,
        },
        changes,
    )
}

#[test]
fn changed_columns_keep_their_server_clauses() -> Result<(), AlterError> {
    let plan = plan(&[
        SchemaChange::ColumnNullable {
            column: "note".into(),
            nullable: false,
        },
        SchemaChange::RenameColumn {
            column: "note".into(),
            name: "memo".into(),
        },
        SchemaChange::ColumnDefault {
            column: "updated".into(),
            default: DefaultValue::Expression("now()".into()),
        },
        SchemaChange::RenameColumn {
            column: "id".into(),
            name: "order_id".into(),
        },
    ])?;
    assert_eq!(
        plan.statements,
        ["ALTER TABLE `shop`.`orders`\n  \
          RENAME COLUMN `id` TO `order_id`,\n  \
          CHANGE COLUMN `note` `memo` varchar(40) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL DEFAULT 'n/a' COMMENT 'free text',\n  \
          MODIFY COLUMN `updated` timestamp NULL DEFAULT (now()) ON UPDATE CURRENT_TIMESTAMP"]
    );
    assert!(!plan.destructive);
    Ok(())
}

#[test]
fn types_columns_indexes_comment_and_rename_form_one_statement() -> Result<(), AlterError> {
    let plan = plan(&[
        SchemaChange::ColumnType {
            column: "note".into(),
            data_type: "text".into(),
        },
        SchemaChange::DropColumn("updated".into()),
        SchemaChange::AddColumn(NewColumn {
            name: "qty".into(),
            data_type: "int unsigned".into(),
            nullable: false,
            default: DefaultValue::Literal("1".into()),
            comment: Some("it's".into()),
        }),
        SchemaChange::DropIndex("note_key".into()),
        SchemaChange::CreateIndex(NewIndex {
            name: None,
            columns: vec!["qty".into(), "note".into()],
            unique: true,
            method: IndexMethod::Hash,
        }),
        SchemaChange::TableComment(Some("orders".into())),
        SchemaChange::RenameTable("purchases".into()),
    ])?;
    assert_eq!(
        plan.statements,
        ["ALTER TABLE `shop`.`orders`\n  \
          DROP COLUMN `updated`,\n  \
          MODIFY COLUMN `note` text DEFAULT 'n/a' COMMENT 'free text',\n  \
          ADD COLUMN `qty` int unsigned NOT NULL DEFAULT '1' COMMENT 'it''s',\n  \
          DROP INDEX `note_key`,\n  \
          ADD UNIQUE INDEX (`qty`, `note`) USING HASH,\n  \
          COMMENT = 'orders',\n  \
          RENAME TO `purchases`"]
    );
    assert!(plan.destructive);
    Ok(())
}

#[test]
fn invalid_changes_are_refused() {
    let gin = plan(&[SchemaChange::CreateIndex(NewIndex {
        name: Some("g".into()),
        columns: vec!["note".into()],
        unique: false,
        method: IndexMethod::Gin,
    })]);
    assert!(matches!(gin, Err(AlterError::Unsupported(_))));
    let primary = plan(&[SchemaChange::DropIndex("PRIMARY".into())]);
    assert_eq!(
        primary.err(),
        Some(AlterError::ConstraintIndex("PRIMARY".into()))
    );
    let injected = plan(&[SchemaChange::ColumnType {
        column: "id".into(),
        data_type: "int; DROP TABLE x".into(),
    }]);
    assert!(matches!(injected, Err(AlterError::InvalidType(_))));
    let mut unreadable = detail();
    if let Some(info) = unreadable.info.as_mut() {
        info.definition = None;
    }
    let result = alter_sql(
        &AlterTable {
            schema: "shop",
            table: "orders",
            detail: &unreadable,
        },
        &[SchemaChange::ColumnNullable {
            column: "id".into(),
            nullable: true,
        }],
    );
    assert!(matches!(result, Err(AlterError::UnreadableDefinition(_))));
}
