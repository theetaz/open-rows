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
fn using_and_concurrent_index_have_safe_boundaries() -> Result<(), AlterError> {
    let detail = detail();
    let using = |expression: &str| SchemaChange::ColumnTypeUsing {
        column: "value".into(),
        data_type: "integer".into(),
        expression: expression.into(),
    };
    assert!(
        plan(&detail, &[using("length(value)")])?.statements[0].ends_with("USING (length(value))")
    );
    for expression in [
        "",
        "value); DROP TABLE t; --",
        "value) --",
        "value, id",
        "value FROM t",
    ] {
        assert!(plan(&detail, &[using(expression)]).is_err(), "{expression}");
    }
    let concurrent = SchemaChange::CreateIndexConcurrently(openrows_driver_api::schema::NewIndex {
        columns: vec!["value".into()],
        ..Default::default()
    });
    let generated = plan(&detail, std::slice::from_ref(&concurrent))?;
    assert_eq!(
        generated.run,
        openrows_driver_api::structure::AlterRun::Autocommit
    );
    assert!(generated.statements[0].contains("INDEX CONCURRENTLY"));
    assert!(plan(&detail, &[concurrent, using("length(value)")]).is_err());
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
    Ok(())
}
