// SPDX-License-Identifier: Apache-2.0
use super::*;
use openrows_driver_api::objects::RoutineKind;
use openrows_driver_api::schema::{
    ColumnDetail, ForeignTarget, NewCheck, NewForeignKey, NewIndex, ReferentialAction, TableInfo,
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
fn new_tables_and_their_indexes() -> Result<(), GenerateError> {
    let definition = NewTable {
        columns: vec![
            NewColumn {
                name: "id".into(),
                data_type: "INTEGER".into(),
                ..NewColumn::default()
            },
            NewColumn {
                name: "title".into(),
                data_type: "TEXT".into(),
                nullable: true,
                collation: Some("NOCASE".into()),
                ..NewColumn::default()
            },
        ],
        primary_key: vec!["id".into()],
        indexes: vec![NewIndex {
            columns: vec!["title".into()],
            predicate: Some("title IS NOT NULL".into()),
            ..NewIndex::default()
        }],
        foreign_keys: vec![NewForeignKey {
            columns: vec!["id".into()],
            target: ForeignTarget {
                schema: String::new(),
                table: "owners".into(),
                columns: vec!["id".into()],
            },
            on_delete: ReferentialAction::Cascade,
            ..NewForeignKey::default()
        }],
        checks: vec![NewCheck {
            name: None,
            expression: "id > 0".into(),
        }],
        comment: None,
    };
    assert_eq!(
        sql(&ObjectChange::CreateTable {
            table: name("main", "notes"),
            definition,
        })?,
        [
            "CREATE TABLE \"main\".\"notes\" (\n    \"id\" INTEGER NOT NULL,\n    \
             \"title\" TEXT COLLATE NOCASE,\n    PRIMARY KEY (\"id\"),\n    \
             FOREIGN KEY (\"id\") REFERENCES \"owners\" (\"id\") ON DELETE CASCADE,\n    \
             CHECK (id > 0)\n)",
            "CREATE INDEX \"main\".\"notes_title_idx\" ON \"notes\" (\"title\") WHERE (title IS NOT NULL)",
        ]
    );
    Ok(())
}

#[test]
fn truncate_deletes_and_views_are_recreated() -> Result<(), GenerateError> {
    let table = name("main", "orders");
    assert_eq!(
        sql(&ObjectChange::TruncateTable {
            table: table.clone(),
            cascade: false
        })?,
        ["DELETE FROM \"main\".\"orders\""]
    );
    assert_eq!(
        sql(&ObjectChange::SaveView {
            view: name("main", "v"),
            query: "SELECT 1".into(),
            materialized: false,
            replace: true,
        })?,
        [
            "DROP VIEW IF EXISTS \"main\".\"v\"",
            "CREATE VIEW \"main\".\"v\" AS\nSELECT 1"
        ]
    );
    assert_eq!(
        sql(&ObjectChange::SaveTrigger {
            table,
            replaces: Some("t".into()),
            definition: "CREATE TRIGGER t AFTER INSERT ON orders BEGIN SELECT 1; END".into(),
        })?,
        [
            "DROP TRIGGER IF EXISTS \"main\".\"t\"",
            "CREATE TRIGGER t AFTER INSERT ON orders BEGIN SELECT 1; END"
        ]
    );
    Ok(())
}

#[test]
fn duplicates_reuse_the_stored_definition() -> Result<(), GenerateError> {
    let detail = TableDetail {
        columns: vec![ColumnDetail {
            name: "id".into(),
            ..ColumnDetail::default()
        }],
        info: Some(TableInfo {
            definition: Some("CREATE TABLE orders (\n    id INTEGER PRIMARY KEY\n);\n".into()),
            ..TableInfo::default()
        }),
        ..TableDetail::default()
    };
    let plan = object_sql(
        &ObjectChange::DuplicateTable {
            table: name("main", "orders"),
            name: "orders_copy".into(),
            with_data: true,
        },
        Some(&detail),
    )?;
    assert_eq!(
        plan.statements,
        [
            "CREATE TABLE \"main\".\"orders_copy\" (\n    id INTEGER PRIMARY KEY\n)",
            "INSERT INTO \"main\".\"orders_copy\" (\"id\") SELECT \"id\" FROM \"main\".\"orders\"",
        ]
    );
    Ok(())
}

#[test]
fn routines_databases_and_schemas_are_unsupported() {
    for change in [
        ObjectChange::DropRoutine {
            routine: name("main", "f"),
            kind: RoutineKind::Function,
            arguments: None,
        },
        ObjectChange::CreateDatabase {
            name: "x".into(),
            encoding: None,
            collation: None,
        },
        ObjectChange::CreateSchema { name: "x".into() },
        ObjectChange::DropTable {
            table: name("main", "t"),
            cascade: true,
        },
    ] {
        assert!(sql(&change).is_err(), "{change:?}");
    }
}
