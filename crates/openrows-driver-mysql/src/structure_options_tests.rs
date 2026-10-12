// SPDX-License-Identifier: Apache-2.0
use super::*;
use openrows_driver_api::schema::{
    ColumnDetail, ConstraintDetail, ConstraintKind, NewKey, TableDetail, TableInfo,
};

fn detail() -> TableDetail {
    TableDetail {
        columns: vec![
            ColumnDetail {
                name: "id".into(),
                data_type: "INTEGER".into(),
                nullable: false,
                ..ColumnDetail::default()
            },
            ColumnDetail {
                name: "value".into(),
                data_type: "VARCHAR(20)".into(),
                nullable: true,
                ..ColumnDetail::default()
            },
        ],
        info: Some(TableInfo {
            definition: Some("CREATE TABLE t (id INTEGER NOT NULL, value VARCHAR(20))".into()),
            ..TableInfo::default()
        }),
        ..TableDetail::default()
    }
}

fn plan(detail: &TableDetail, changes: &[SchemaChange]) -> Result<AlterPlan, AlterError> {
    alter_sql(
        &AlterTable {
            schema: "main",
            table: "t",
            detail,
        },
        changes,
    )
}

#[test]
fn keys_are_previewed_and_invalid_keys_are_refused() -> Result<(), AlterError> {
    let mut detail = detail();
    let key = |columns: Vec<String>| NewKey {
        name: Some("key".into()),
        columns,
    };
    let primary = SchemaChange::AddPrimaryKey(key(vec!["id".into()]));
    let unique = SchemaChange::AddUnique(NewKey {
        name: Some("uq".into()),
        columns: vec!["value".into()],
    });
    let generated = plan(&detail, &[primary.clone(), unique])?;
    assert!(
        generated
            .statements
            .iter()
            .any(|s| s.contains("PRIMARY KEY"))
    );
    assert!(generated.statements.iter().any(|s| s.contains("UNIQUE")));
    assert_eq!(
        generated.run,
        openrows_driver_api::structure::AlterRun::StrictMySql
    );
    assert!(generated.statements[0].ends_with("ALGORITHM=INPLACE"));
    assert!(!generated.warnings.is_empty());
    assert!(
        plan(
            &detail,
            &[
                SchemaChange::AddUnique(key(vec!["id".into()])),
                SchemaChange::AddCheck(openrows_driver_api::schema::NewCheck {
                    name: Some("key".into()),
                    expression: "id > 0".into()
                }),
            ]
        )
        .is_err()
    );
    for columns in [
        vec![],
        vec!["missing".into()],
        vec!["id".into(), "id".into()],
    ] {
        assert!(plan(&detail, &[SchemaChange::AddUnique(key(columns))]).is_err());
    }
    assert!(plan(&detail, &[primary.clone(), primary.clone()]).is_err());
    assert!(
        plan(
            &detail,
            &[
                primary.clone(),
                SchemaChange::ColumnNullable {
                    column: "id".into(),
                    nullable: true
                }
            ]
        )
        .is_err()
    );
    detail.constraints.push(ConstraintDetail {
        name: "pk".into(),
        kind: ConstraintKind::Primary,
        definition: "PRIMARY KEY (id)".into(),
        columns: vec!["id".into()],
        references: None,
    });
    assert!(plan(&detail, &[primary]).is_err());
    Ok(())
}

#[test]
fn postgres_only_options_are_refused() {
    let detail = detail();
    assert!(
        plan(
            &detail,
            &[SchemaChange::ColumnTypeUsing {
                column: "value".into(),
                data_type: "integer".into(),
                expression: "length(value)".into()
            }]
        )
        .is_err()
    );
    assert!(
        plan(
            &detail,
            &[SchemaChange::CreateIndexConcurrently(
                openrows_driver_api::schema::NewIndex {
                    columns: vec!["id".into()],
                    ..Default::default()
                }
            )]
        )
        .is_err()
    );
}

#[test]
fn reordering_preserves_clauses_and_uses_final_names() -> Result<(), AlterError> {
    let mut detail = detail();
    detail.info.as_mut().ok_or(AlterError::NotATable)?.definition = Some("CREATE TABLE t (id INTEGER NOT NULL, value VARCHAR(20) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin DEFAULT 'x' COMMENT 'keep')".into());
    let first = plan(
        &detail,
        &[SchemaChange::ReorderColumn {
            column: "value".into(),
            after: None,
        }],
    )?;
    assert!(first.statements[0].contains("COMMENT 'keep' FIRST"));
    assert!(first.statements[0].contains("COLLATE utf8mb4_bin"));
    let after = plan(
        &detail,
        &[
            SchemaChange::RenameColumn {
                column: "id".into(),
                name: "key_id".into(),
            },
            SchemaChange::ReorderColumn {
                column: "value".into(),
                after: Some("id".into()),
            },
        ],
    )?;
    assert!(after.statements[0].contains("AFTER `key_id`"));
    let combined = plan(
        &detail,
        &[
            SchemaChange::ColumnType {
                column: "value".into(),
                data_type: "VARCHAR(50)".into(),
            },
            SchemaChange::RenameColumn {
                column: "value".into(),
                name: "renamed".into(),
            },
            SchemaChange::ReorderColumn {
                column: "value".into(),
                after: None,
            },
        ],
    )?;
    assert_eq!(combined.statements[0].matches("CHANGE COLUMN").count(), 1);
    assert!(!combined.statements[0].contains("MODIFY COLUMN"));
    assert!(combined.statements[0].contains("CHANGE COLUMN `value` `renamed` VARCHAR(50)"));
    for anchor in ["value", "missing"] {
        assert!(
            plan(
                &detail,
                &[SchemaChange::ReorderColumn {
                    column: "value".into(),
                    after: Some(anchor.into())
                }]
            )
            .is_err()
        );
    }
    Ok(())
}
