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
        openrows_driver_api::structure::AlterRun::Transaction
    );
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
            &[SchemaChange::ReorderColumn {
                column: "value".into(),
                after: None
            }]
        )
        .is_err()
    );
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
            &[SchemaChange::CreateIndexConcurrently(NewIndex {
                columns: vec!["id".into()],
                ..Default::default()
            })]
        )
        .is_err()
    );
}

#[test]
fn primary_key_nullability_is_previewed_before_the_key() -> Result<(), AlterError> {
    let mut detail = detail();
    detail.columns[0].nullable = true;
    let generated = plan(
        &detail,
        &[SchemaChange::AddPrimaryKey(NewKey {
            name: None,
            columns: vec!["id".into()],
        })],
    )?;
    assert_eq!(
        generated.statements[0],
        "ALTER TABLE [main].[t] ALTER COLUMN [id] INTEGER NOT NULL"
    );
    assert!(generated.statements[1].contains("PRIMARY KEY"));
    assert!(generated.warnings.iter().any(|w| w.contains("NULL")));
    Ok(())
}
