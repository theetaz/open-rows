// SPDX-License-Identifier: Apache-2.0
use super::*;
use openrows_driver_api::schema::{
    ColumnDetail, ConstraintDetail, ConstraintKind, IndexDetail, RelationKind, TableDetail,
    TableInfo, TriggerDetail, TriggerTiming,
};
use openrows_sql::ddl::TableDefinition;

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
            ..NewColumn::default()
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
            ..NewIndex::default()
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
        ..NewColumn::default()
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
        ..NewIndex::default()
    })]);
    assert!(matches!(gin, Err(AlterError::Unsupported(_))));
}

#[test]
fn constraints_and_collations_rebuild_the_table() -> Result<(), AlterError> {
    use openrows_driver_api::schema::{ForeignTarget, NewCheck, NewForeignKey};
    let mut detail = detail();
    detail.constraints.push(ConstraintDetail {
        name: "fk_orders_0".into(),
        kind: ConstraintKind::Foreign,
        definition: String::new(),
        columns: vec!["customer".into()],
        references: None,
    });
    let rebuilt = alter_sql(
        &AlterTable {
            schema: "main",
            table: "orders",
            detail: &detail,
        },
        &[
            SchemaChange::DropConstraint("fk_orders_0".into()),
            SchemaChange::ColumnCollation {
                column: "note".into(),
                charset: None,
                collation: Some("NOCASE".into()),
            },
            SchemaChange::AddForeignKey(NewForeignKey {
                name: Some("by_note".into()),
                columns: vec!["note".into()],
                target: ForeignTarget {
                    schema: String::new(),
                    table: "notes".into(),
                    columns: vec!["text".into()],
                },
                ..NewForeignKey::default()
            }),
            SchemaChange::AddCheck(NewCheck {
                name: None,
                expression: "total >= 0".into(),
            }),
        ],
    )?;
    assert_eq!(
        rebuilt.statements[0],
        "CREATE TABLE \"main\".\"openrows_rebuild_orders\" (\n    \
         id INTEGER PRIMARY KEY,\n    customer INTEGER,\n    \
         note TEXT COLLATE NOCASE DEFAULT 'n/a',\n    total REAL NOT NULL,\n    \
         CONSTRAINT \"by_note\" FOREIGN KEY (\"note\") REFERENCES \"notes\" (\"text\"),\n    \
         CHECK (total >= 0)\n)"
    );
    let charset = plan(&[SchemaChange::ColumnCollation {
        column: "note".into(),
        charset: Some("utf8".into()),
        collation: None,
    }]);
    assert!(matches!(charset, Err(AlterError::Unsupported(_))));
    Ok(())
}

#[test]
fn checks_are_listed_and_dropped_by_position_or_name() -> Result<(), &'static str> {
    let definition = TableDefinition::parse(
        "CREATE TABLE t (a INT, CHECK (a > 0), CONSTRAINT small CHECK (a < 9))",
        DIALECT,
    )
    .ok_or("parse")?;
    let checks = table_checks("t", &definition);
    let names: Vec<&str> = checks.iter().map(|check| check.name.as_str()).collect();
    assert_eq!(names, ["ck_t_1", "small"]);
    assert_eq!(checks[1].text, "CONSTRAINT small CHECK (a < 9)");
    Ok(())
}
