// SPDX-License-Identifier: Apache-2.0
use super::*;
use openrows_driver_api::schema::{
    ColumnDetail, IndexDetail, IndexMethod, NewIndex, RelationKind, TableDetail, TableInfo,
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
            ..NewColumn::default()
        }),
        SchemaChange::DropIndex("note_key".into()),
        SchemaChange::CreateIndex(NewIndex {
            name: None,
            columns: vec!["qty".into(), "note".into()],
            unique: true,
            method: IndexMethod::Hash,
            ..NewIndex::default()
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
        ..NewIndex::default()
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

#[test]
fn numbering_collations_and_constraints() -> Result<(), AlterError> {
    use openrows_driver_api::schema::{
        ConstraintDetail, ConstraintKind, ForeignTarget, Identity, NewCheck, NewForeignKey,
        ReferentialAction,
    };
    let mut detail = detail();
    let constraint = |name: &str, kind| ConstraintDetail {
        name: name.into(),
        kind,
        definition: String::new(),
        columns: Vec::new(),
        references: None,
    };
    detail.constraints = vec![
        constraint("orders_fk", ConstraintKind::Foreign),
        constraint("orders_chk", ConstraintKind::Check),
    ];
    detail.columns[1].data_type = "varchar(40)".into();
    let altered = alter_sql(
        &AlterTable {
            schema: "shop",
            table: "orders",
            detail: &detail,
        },
        &[
            SchemaChange::ColumnIdentity {
                column: "id".into(),
                identity: Identity::None,
            },
            SchemaChange::ColumnCollation {
                column: "note".into(),
                charset: Some("latin1".into()),
                collation: None,
            },
            SchemaChange::ColumnType {
                column: "note".into(),
                data_type: "varchar(10)".into(),
            },
            SchemaChange::DropConstraint("orders_fk".into()),
            SchemaChange::DropConstraint("orders_chk".into()),
            SchemaChange::AddForeignKey(NewForeignKey {
                name: Some("orders_owner".into()),
                columns: vec!["id".into()],
                target: ForeignTarget {
                    schema: "crm".into(),
                    table: "owners".into(),
                    columns: vec!["id".into()],
                },
                on_delete: ReferentialAction::Cascade,
                on_update: ReferentialAction::NoAction,
            }),
            SchemaChange::AddCheck(NewCheck {
                name: None,
                expression: "id > 0".into(),
            }),
        ],
    )?;
    assert_eq!(
        altered.statements,
        ["ALTER TABLE `shop`.`orders`\n  \
          DROP FOREIGN KEY `orders_fk`,\n  \
          DROP CONSTRAINT `orders_chk`,\n  \
          MODIFY COLUMN `id` bigint unsigned NOT NULL,\n  \
          MODIFY COLUMN `note` varchar(10) CHARACTER SET latin1 DEFAULT 'n/a' COMMENT 'free text',\n  \
          ADD CONSTRAINT `orders_owner` FOREIGN KEY (`id`) REFERENCES `crm`.`owners` (`id`) ON DELETE CASCADE,\n  \
          ADD CHECK (id > 0)"]
    );
    assert!(
        altered
            .warnings
            .iter()
            .any(|w| w.contains("ALGORITHM=COPY"))
    );
    assert!(
        altered
            .warnings
            .iter()
            .any(|w| w.contains("from varchar(40) to varchar(10)"))
    );
    Ok(())
}

#[test]
fn identity_columns_and_partial_indexes_are_refused() {
    let identity = plan(&[SchemaChange::ColumnIdentity {
        column: "id".into(),
        identity: Identity::Always,
    }]);
    assert!(matches!(identity, Err(AlterError::Unsupported(_))));
    let partial = plan(&[SchemaChange::CreateIndex(NewIndex {
        columns: vec!["note".into()],
        predicate: Some("note IS NOT NULL".into()),
        ..NewIndex::default()
    })]);
    assert!(matches!(partial, Err(AlterError::Unsupported(_))));
}
