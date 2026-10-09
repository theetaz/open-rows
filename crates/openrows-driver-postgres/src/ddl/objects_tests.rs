// SPDX-License-Identifier: Apache-2.0
use super::*;
use openrows_driver_api::schema::{
    ColumnDetail, DefaultValue, ForeignTarget, Identity, NewCheck, NewColumn, NewForeignKey,
    NewIndex, ReferentialAction,
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
fn new_tables_have_keys_constraints_indexes_and_comments() -> Result<(), GenerateError> {
    let definition = NewTable {
        columns: vec![
            NewColumn {
                name: "id".into(),
                data_type: "bigint".into(),
                identity: Identity::Always,
                ..NewColumn::default()
            },
            NewColumn {
                name: "owner_id".into(),
                data_type: "integer".into(),
                nullable: true,
                ..NewColumn::default()
            },
            NewColumn {
                name: "title".into(),
                data_type: "varchar(80)".into(),
                default: DefaultValue::Literal("untitled".into()),
                collation: Some("C".into()),
                comment: Some("Shown in lists".into()),
                ..NewColumn::default()
            },
        ],
        primary_key: vec!["id".into()],
        indexes: vec![NewIndex {
            columns: vec!["title".into()],
            descending: vec!["title".into()],
            predicate: Some("owner_id IS NOT NULL".into()),
            ..NewIndex::default()
        }],
        foreign_keys: vec![NewForeignKey {
            name: Some("notes_owner_fk".into()),
            columns: vec!["owner_id".into()],
            target: ForeignTarget {
                schema: String::new(),
                table: "owners".into(),
                columns: vec!["id".into()],
            },
            on_delete: ReferentialAction::Cascade,
            on_update: ReferentialAction::NoAction,
        }],
        checks: vec![NewCheck {
            name: None,
            expression: "length(title) > 0".into(),
        }],
        comment: Some("Notes".into()),
    };
    let statements = sql(&ObjectChange::CreateTable {
        table: name("app", "notes"),
        definition,
    })?;
    assert_eq!(
        statements,
        [
            "CREATE TABLE \"app\".\"notes\" (\n    \
             \"id\" bigint GENERATED ALWAYS AS IDENTITY NOT NULL,\n    \
             \"owner_id\" integer,\n    \
             \"title\" varchar(80) COLLATE \"C\" NOT NULL DEFAULT 'untitled',\n    \
             PRIMARY KEY (\"id\"),\n    \
             CONSTRAINT \"notes_owner_fk\" FOREIGN KEY (\"owner_id\") REFERENCES \"app\".\"owners\" (\"id\") ON DELETE CASCADE,\n    \
             CHECK (length(title) > 0)\n)",
            "CREATE INDEX ON \"app\".\"notes\" USING btree (\"title\" DESC) WHERE (owner_id IS NOT NULL)",
            "COMMENT ON TABLE \"app\".\"notes\" IS 'Notes'",
            "COMMENT ON COLUMN \"app\".\"notes\".\"title\" IS 'Shown in lists'",
        ]
    );
    Ok(())
}

#[test]
fn invalid_tables_and_auto_increment_are_refused() {
    let empty = ObjectChange::CreateTable {
        table: name("public", "t"),
        definition: NewTable::default(),
    };
    assert!(sql(&empty).is_err());
    let auto = ObjectChange::CreateTable {
        table: name("public", "t"),
        definition: NewTable {
            columns: vec![NewColumn {
                name: "id".into(),
                data_type: "int".into(),
                identity: Identity::AutoIncrement,
                ..NewColumn::default()
            }],
            ..NewTable::default()
        },
    };
    assert!(sql(&auto).is_err_and(|error| error.0.contains("identity")));
}

#[test]
fn table_lifecycle_statements() -> Result<(), GenerateError> {
    let table = name("public", "orders");
    assert_eq!(
        sql(&ObjectChange::RenameTable {
            table: table.clone(),
            name: "sales".into()
        })?,
        ["ALTER TABLE \"public\".\"orders\" RENAME TO \"sales\""]
    );
    let truncate = object_sql(
        &ObjectChange::TruncateTable {
            table: table.clone(),
            cascade: true,
        },
        None,
    )?;
    assert_eq!(
        truncate.statements,
        ["TRUNCATE TABLE \"public\".\"orders\" CASCADE"]
    );
    assert!(truncate.destructive);
    assert_eq!(
        sql(&ObjectChange::DropTable {
            table,
            cascade: false
        })?,
        ["DROP TABLE \"public\".\"orders\""]
    );
    Ok(())
}

#[test]
fn duplicates_copy_rows_without_generated_columns() -> Result<(), GenerateError> {
    let column = |name: &str, generated| ColumnDetail {
        name: name.into(),
        generated,
        ..ColumnDetail::default()
    };
    let detail = TableDetail {
        columns: vec![
            column("id", Generated::IdentityAlways),
            column("total", Generated::No),
            column("tax", Generated::Stored),
        ],
        ..TableDetail::default()
    };
    let change = ObjectChange::DuplicateTable {
        table: name("public", "orders"),
        name: "orders_copy".into(),
        with_data: true,
    };
    let plan = object_sql(&change, Some(&detail))?;
    assert_eq!(
        plan.statements,
        [
            "CREATE TABLE \"public\".\"orders_copy\" (LIKE \"public\".\"orders\" INCLUDING ALL)",
            "INSERT INTO \"public\".\"orders_copy\" (\"id\", \"total\") OVERRIDING SYSTEM VALUE SELECT \"id\", \"total\" FROM \"public\".\"orders\"",
        ]
    );
    assert!(object_sql(&change, None).is_err());
    Ok(())
}

#[test]
fn views_replace_and_materialized_views_rebuild() -> Result<(), GenerateError> {
    let view = name("public", "recent");
    let replace = sql(&ObjectChange::SaveView {
        view: view.clone(),
        query: "SELECT * FROM orders;".into(),
        materialized: false,
        replace: true,
    })?;
    assert_eq!(
        replace,
        ["CREATE OR REPLACE VIEW \"public\".\"recent\" AS\nSELECT * FROM orders"]
    );
    let rebuilt = object_sql(
        &ObjectChange::SaveView {
            view: view.clone(),
            query: "SELECT 1".into(),
            materialized: true,
            replace: true,
        },
        None,
    )?;
    assert_eq!(rebuilt.statements.len(), 2);
    assert!(rebuilt.destructive);
    let smuggled = ObjectChange::SaveView {
        view,
        query: "SELECT 1; DROP TABLE orders".into(),
        materialized: false,
        replace: false,
    };
    assert!(sql(&smuggled).is_err());
    Ok(())
}

#[test]
fn triggers_and_routines_use_checked_definitions() -> Result<(), GenerateError> {
    let table = name("public", "orders");
    let trigger = sql(&ObjectChange::SaveTrigger {
        table: table.clone(),
        replaces: Some("stamp".into()),
        definition:
            "CREATE TRIGGER stamp BEFORE UPDATE ON orders FOR EACH ROW EXECUTE FUNCTION touch();"
                .into(),
    })?;
    assert_eq!(
        trigger,
        [
            "DROP TRIGGER IF EXISTS \"stamp\" ON \"public\".\"orders\"",
            "CREATE TRIGGER stamp BEFORE UPDATE ON orders FOR EACH ROW EXECUTE FUNCTION touch()",
        ]
    );
    let not_a_routine = ObjectChange::SaveRoutine {
        routine: name("public", "f"),
        kind: RoutineKind::Function,
        replaces: None,
        definition: "DROP TABLE orders".into(),
    };
    assert!(sql(&not_a_routine).is_err());
    assert_eq!(
        sql(&ObjectChange::DropRoutine {
            routine: name("public", "touch"),
            kind: RoutineKind::Function,
            arguments: Some("integer, text".into()),
        })?,
        ["DROP FUNCTION \"public\".\"touch\"(integer, text)"]
    );
    assert_eq!(
        sql(&ObjectChange::DropTrigger {
            table,
            name: "stamp".into()
        })?,
        ["DROP TRIGGER \"stamp\" ON \"public\".\"orders\""]
    );
    Ok(())
}

#[test]
fn databases_run_from_the_home_database() -> Result<(), GenerateError> {
    let create = object_sql(
        &ObjectChange::CreateDatabase {
            name: "shop".into(),
            encoding: Some("UTF8".into()),
            collation: Some("en_US.UTF-8".into()),
        },
        None,
    )?;
    assert_eq!(
        create.statements,
        [
            "CREATE DATABASE \"shop\" ENCODING 'UTF8' LC_COLLATE 'en_US.UTF-8' LC_CTYPE 'en_US.UTF-8' TEMPLATE template0"
        ]
    );
    assert_eq!(create.run, PlanRun::HomeDatabase);
    let drop = object_sql(
        &ObjectChange::DropDatabase {
            name: "shop".into(),
        },
        None,
    )?;
    assert!(drop.destructive && drop.run == PlanRun::HomeDatabase);
    assert!(
        sql(&ObjectChange::DropDatabase {
            name: "postgres".into()
        })
        .is_err()
    );
    assert_eq!(
        sql(&ObjectChange::RenameSchema {
            name: "a".into(),
            to: "b".into()
        })?,
        ["ALTER SCHEMA \"a\" RENAME TO \"b\""]
    );
    Ok(())
}
