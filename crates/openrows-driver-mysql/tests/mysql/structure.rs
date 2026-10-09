// SPDX-License-Identifier: Apache-2.0
//! Structure changes as one `ALTER TABLE` that keeps server-side clauses.
use openrows_driver_api::schema::{
    DefaultValue, IndexMethod, NewColumn, NewIndex, SchemaChange, TableDetail,
};
use openrows_driver_api::structure::AlterTable;
use openrows_driver_api::{Catalog, Driver, Editor};
use openrows_driver_mysql::{Flavour, MySqlDriver, MySqlSession};

use super::support::{DATABASE, TestResult, run, session};

const TABLE: &str = "alter_orders";

async fn detail(
    session: &mut MySqlSession,
    table: &str,
) -> Result<TableDetail, Box<dyn std::error::Error>> {
    Ok(session.table_detail(DATABASE, table).await?)
}

fn changes() -> Vec<SchemaChange> {
    vec![
        SchemaChange::RenameColumn {
            column: "note".into(),
            name: "memo".into(),
        },
        SchemaChange::ColumnNullable {
            column: "note".into(),
            nullable: false,
        },
        SchemaChange::ColumnType {
            column: "qty".into(),
            data_type: "bigint".into(),
        },
        SchemaChange::ColumnDefault {
            column: "qty".into(),
            default: DefaultValue::Literal("5".into()),
        },
        SchemaChange::AddColumn(NewColumn {
            name: "tag".into(),
            data_type: "varchar(10)".into(),
            nullable: true,
            default: DefaultValue::None,
            comment: Some("label".into()),
            ..NewColumn::default()
        }),
        SchemaChange::DropIndex("by_qty".into()),
        SchemaChange::CreateIndex(NewIndex {
            name: Some("by_memo".into()),
            columns: vec!["memo".into(), "tag".into()],
            unique: false,
            method: IndexMethod::Btree,
            ..NewIndex::default()
        }),
        SchemaChange::TableComment(Some("orders".into())),
    ]
}

#[tokio::test]
#[ignore = "needs MySQL or MariaDB; run with `just test-mysql`"]
async fn alter_table_keeps_charset_auto_increment_and_on_update() -> TestResult {
    let mut session = session().await?;
    run(
        &mut session,
        &[
            "DROP TABLE IF EXISTS alter_orders",
            "CREATE TABLE alter_orders (id INT AUTO_INCREMENT PRIMARY KEY, \
             note VARCHAR(40) CHARACTER SET latin1 DEFAULT 'n/a' COMMENT 'free text', \
             qty INT NOT NULL DEFAULT 1, \
             touched TIMESTAMP NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP, \
             KEY by_qty (qty))",
            "INSERT INTO alter_orders (note, qty) VALUES ('a', 2)",
        ],
    )
    .await?;
    let before = detail(&mut session, TABLE).await?;
    let plan = MySqlDriver(Flavour::MySql).alter_sql(
        &AlterTable {
            schema: DATABASE,
            table: TABLE,
            detail: &before,
        },
        &changes(),
    )?;
    assert_eq!(plan.statements.len(), 1);
    session.commit_ddl(&plan.statements).await?;
    let after = detail(&mut session, TABLE).await?;
    let names: Vec<_> = after
        .columns
        .iter()
        .map(|column| column.name.as_str())
        .collect();
    assert_eq!(names, ["id", "memo", "qty", "touched", "tag"]);
    let memo = &after.columns[1];
    assert!(!memo.nullable);
    assert_eq!(memo.comment.as_deref(), Some("free text"));
    assert!(after.columns[2].data_type.starts_with("bigint"));
    assert_eq!(after.columns[2].default.as_deref(), Some("5"));
    let definition = after
        .info
        .as_ref()
        .and_then(|info| info.definition.clone())
        .unwrap_or_default()
        .to_lowercase();
    assert!(definition.contains("character set latin1"), "{definition}");
    assert!(definition.contains("auto_increment"), "{definition}");
    assert!(
        definition.contains("on update current_timestamp"),
        "{definition}"
    );
    assert!(after.indexes.iter().any(|index| index.name == "by_memo"));
    assert!(!after.indexes.iter().any(|index| index.name == "by_qty"));
    assert_eq!(
        after.info.as_ref().and_then(|info| info.comment.as_deref()),
        Some("orders")
    );
    run(&mut session, &["DROP TABLE alter_orders"]).await?;
    Ok(())
}
