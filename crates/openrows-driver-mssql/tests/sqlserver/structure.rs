// SPDX-License-Identifier: Apache-2.0
//! Generated structure changes commit in one transaction and read back.
use openrows_driver_api::schema::{
    DefaultValue, IndexMethod, NewColumn, NewIndex, SchemaChange, TableDetail,
};
use openrows_driver_api::structure::AlterTable;
use openrows_driver_api::{Catalog, Driver, Editor, Session};
use openrows_driver_mssql::SqlServerDriver;

use super::support::{Collect, TestResult, fixture, run, session};

#[tokio::test]
#[ignore = "needs SQL Server; run with `just test-mssql`"]
async fn change_sets_alter_rename_default_comment_and_index() -> TestResult {
    let mut session = session().await?;
    let table = fixture("alter");
    run(
        &mut session,
        &[
            &format!(
                "CREATE TABLE dbo.{table} (id int PRIMARY KEY, \
                 name varchar(40) COLLATE Latin1_General_BIN NOT NULL, \
                 qty int CONSTRAINT df_{table}_qty DEFAULT 5, note nvarchar(10))"
            ),
            &format!("INSERT INTO dbo.{table} (id, name, note) VALUES (1, 'one', N'a')"),
        ],
    )
    .await?;
    let detail = session.table_detail("dbo", &table).await?;
    let target = AlterTable {
        schema: "dbo",
        table: &table,
        detail: &detail,
    };
    let plan = SqlServerDriver.alter_sql(&target, &changes())?;
    session.commit_ddl(&plan.statements).await?;
    let after = session.table_detail("dbo", &table).await?;
    check(&after, &table);
    let mut sink = Collect::default();
    session
        .execute(&format!("SELECT memo, added FROM dbo.{table}"), &mut sink)
        .await?;
    assert_eq!(sink.rows(), 1);
    run(&mut session, &[&format!("DROP TABLE dbo.{table}")]).await?;
    Ok(())
}

fn changes() -> Vec<SchemaChange> {
    vec![
        SchemaChange::ColumnNullable {
            column: "name".into(),
            nullable: true,
        },
        SchemaChange::ColumnDefault {
            column: "qty".into(),
            default: DefaultValue::Literal("7".into()),
        },
        SchemaChange::ColumnComment {
            column: "qty".into(),
            comment: Some("how many".into()),
        },
        SchemaChange::RenameColumn {
            column: "note".into(),
            name: "memo".into(),
        },
        SchemaChange::AddColumn(NewColumn {
            name: "added".into(),
            data_type: "nvarchar(max)".into(),
            nullable: true,
            default: DefaultValue::Literal("x".into()),
            comment: None,
            ..NewColumn::default()
        }),
        SchemaChange::CreateIndex(NewIndex {
            name: None,
            columns: vec!["memo".into()],
            unique: false,
            method: IndexMethod::Btree,
            ..NewIndex::default()
        }),
    ]
}

/// The table reads back as staged.
fn check(after: &TableDetail, table: &str) {
    let columns: Vec<_> = after
        .columns
        .iter()
        .map(|c| {
            (
                c.name.as_str(),
                c.nullable,
                c.default.as_deref(),
                c.comment.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        columns,
        [
            ("id", false, None, None),
            ("name", true, None, None),
            ("qty", true, Some("(N'7')"), Some("how many")),
            ("memo", true, None, None),
            ("added", true, Some("(N'x')"), None),
        ]
    );
    let ddl = after
        .info
        .as_ref()
        .and_then(|info| info.definition.clone())
        .unwrap_or_default();
    assert!(ddl.contains("COLLATE Latin1_General_BIN NULL"), "{ddl}");
    assert!(
        after
            .indexes
            .iter()
            .any(|i| i.name == format!("IX_{table}_memo"))
    );
}
