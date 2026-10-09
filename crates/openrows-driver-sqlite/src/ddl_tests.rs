// SPDX-License-Identifier: Apache-2.0
use super::*;
use openrows_driver_api::schema::{
    ColumnDetail, ConstraintDetail, ConstraintKind, IndexDetail, RelationKind, TableDetail,
    TableInfo, TriggerDetail, TriggerTiming,
};

const CREATE: &str = "CREATE TABLE orders (
    id INTEGER PRIMARY KEY,
    customer INTEGER REFERENCES customers(id) ON DELETE CASCADE,
    note TEXT DEFAULT 'n/a',
    total REAL NOT NULL
);
CREATE INDEX orders_customer ON orders (customer);
CREATE TRIGGER orders_touch AFTER UPDATE ON orders BEGIN SELECT 1; END;
";

fn detail() -> TableDetail {
    let column = |name: &str| ColumnDetail {
        name: name.into(),
        nullable: true,
        ..ColumnDetail::default()
    };
    TableDetail {
        columns: vec![
            column("id"),
            column("customer"),
            column("note"),
            column("total"),
        ],
        constraints: vec![ConstraintDetail {
            name: "PRIMARY KEY".into(),
            kind: ConstraintKind::Primary,
            definition: "PRIMARY KEY (id)".into(),
            columns: vec!["id".into()],
            references: None,
        }],
        indexes: vec![IndexDetail {
            name: "orders_customer".into(),
            columns: "customer".into(),
            definition: "CREATE INDEX orders_customer ON orders (customer)".into(),
            ..IndexDetail::default()
        }],
        partitions: Vec::new(),
        triggers: vec![TriggerDetail {
            name: "orders_touch".into(),
            timing: TriggerTiming::After,
            events: Vec::new(),
            row_level: true,
            function: String::new(),
            enabled: true,
            definition: "CREATE TRIGGER orders_touch AFTER UPDATE ON orders BEGIN SELECT 1; END"
                .into(),
        }],
        info: Some(TableInfo {
            kind: RelationKind::Table,
            definition: Some(CREATE.into()),
            ..TableInfo::default()
        }),
    }
}

fn plan(changes: &[SchemaChange]) -> Result<AlterPlan, AlterError> {
    let detail = detail();
    alter_sql(
        &AlterTable {
            schema: "main",
            table: "orders",
            detail: &detail,
        },
        changes,
    )
}

#[test]
fn renames_plain_drops_and_adds_use_alter_table() -> Result<(), AlterError> {
    let plan = plan(&[
        SchemaChange::DropColumn("note".into()),
        SchemaChange::AddColumn(NewColumn {
            name: "qty".into(),
            data_type: "INTEGER".into(),
            nullable: false,
            default: DefaultValue::Literal("1".into()),
            comment: None,
        }),
        SchemaChange::RenameColumn {
            column: "total".into(),
            name: "amount".into(),
        },
        SchemaChange::CreateIndex(NewIndex {
            name: None,
            columns: vec!["amount".into()],
            unique: true,
            method: IndexMethod::Btree,
        }),
        SchemaChange::DropIndex("orders_customer".into()),
        SchemaChange::RenameTable("purchases".into()),
    ])?;
    assert_eq!(
        plan.statements,
        [
            "ALTER TABLE \"main\".\"orders\" DROP COLUMN \"note\"",
            "ALTER TABLE \"main\".\"orders\" ADD COLUMN \"qty\" INTEGER NOT NULL DEFAULT '1'",
            "ALTER TABLE \"main\".\"orders\" RENAME COLUMN \"total\" TO \"amount\"",
            "DROP INDEX \"main\".\"orders_customer\"",
            "CREATE UNIQUE INDEX \"main\".\"orders_amount_idx\" ON \"orders\" (\"amount\")",
            "ALTER TABLE \"main\".\"orders\" RENAME TO \"purchases\"",
        ]
    );
    assert!(plan.destructive);
    Ok(())
}

#[test]
fn type_and_nullability_changes_rebuild_the_table() -> Result<(), AlterError> {
    let plan = plan(&[
        SchemaChange::ColumnType {
            column: "note".into(),
            data_type: "VARCHAR(20)".into(),
        },
        SchemaChange::ColumnNullable {
            column: "total".into(),
            nullable: true,
        },
        SchemaChange::DropColumn("customer".into()),
        SchemaChange::RenameColumn {
            column: "id".into(),
            name: "order_id".into(),
        },
    ])?;
    assert_eq!(
        plan.statements,
        [
            "CREATE TABLE \"main\".\"openrows_rebuild_orders\" (\n    \
             id INTEGER PRIMARY KEY,\n    note VARCHAR(20) DEFAULT 'n/a',\n    total REAL\n)",
            "INSERT INTO \"main\".\"openrows_rebuild_orders\" (\"id\", \"note\", \"total\") \
             SELECT \"id\", \"note\", \"total\" FROM \"main\".\"orders\"",
            "DROP TABLE \"main\".\"orders\"",
            "PRAGMA legacy_alter_table = ON",
            "ALTER TABLE \"main\".\"openrows_rebuild_orders\" RENAME TO \"orders\"",
            "PRAGMA legacy_alter_table = OFF",
            "CREATE TRIGGER \"main\".orders_touch AFTER UPDATE ON orders BEGIN SELECT 1; END",
            "ALTER TABLE \"main\".\"orders\" RENAME COLUMN \"id\" TO \"order_id\"",
        ]
    );
    assert!(plan.destructive);
    assert!(
        plan.warnings
            .iter()
            .any(|warning| warning.contains("rebuilt"))
    );
    Ok(())
}

#[test]
fn expression_defaults_and_not_null_adds_rebuild_too() -> Result<(), AlterError> {
    let plan = plan(&[SchemaChange::AddColumn(NewColumn {
        name: "at".into(),
        data_type: "TEXT".into(),
        nullable: true,
        default: DefaultValue::Expression("datetime('now')".into()),
        comment: None,
    })])?;
    assert!(
        plan.statements[0]
            .ends_with("total REAL NOT NULL,\n    \"at\" TEXT DEFAULT (datetime('now'))\n)")
    );
    assert_eq!(
        plan.statements[6],
        "CREATE INDEX \"main\".orders_customer ON orders (customer)"
    );
    Ok(())
}

#[test]
fn comments_and_other_index_methods_are_unsupported() {
    let comment = plan(&[SchemaChange::TableComment(Some("x".into()))]);
    assert!(matches!(comment, Err(AlterError::Unsupported(_))));
    let gin = plan(&[SchemaChange::CreateIndex(NewIndex {
        name: Some("g".into()),
        columns: vec!["note".into()],
        unique: false,
        method: IndexMethod::Gin,
    })]);
    assert!(matches!(gin, Err(AlterError::Unsupported(_))));
}
