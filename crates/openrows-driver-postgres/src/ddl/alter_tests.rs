// SPDX-License-Identifier: Apache-2.0
use super::*;
use openrows_driver_api::schema::{
    ColumnDetail, DefaultValue, IndexDetail, IndexMethod, NewColumn, NewIndex, RelationKind,
    TableInfo,
};

fn detail() -> TableDetail {
    let column = |name: &str, data_type: &str| ColumnDetail {
        name: name.into(),
        data_type: data_type.into(),
        nullable: true,
        ..ColumnDetail::default()
    };
    TableDetail {
        columns: vec![
            column("id", "integer"),
            column("name", "text"),
            column("Mixed \"Case\"", "text"),
            column("old", "text"),
        ],
        indexes: vec![
            IndexDetail {
                name: "users_pkey".into(),
                constraint: true,
                ..IndexDetail::default()
            },
            IndexDetail {
                name: "users_name_idx".into(),
                ..IndexDetail::default()
            },
        ],
        info: Some(TableInfo::default()),
        ..TableDetail::default()
    }
}

fn plan(changes: &[SchemaChange]) -> Result<AlterPlan, AlterError> {
    let detail = detail();
    alter_sql(
        &AlterTable {
            schema: "public",
            table: "users",
            detail: &detail,
        },
        changes,
    )
}

fn sql(changes: &[SchemaChange]) -> Result<Vec<String>, AlterError> {
    plan(changes).map(|plan| plan.statements)
}

fn column(column: &str) -> String {
    column.to_owned()
}

#[test]
fn column_changes_quote_identifiers_and_run_rename_last() -> Result<(), AlterError> {
    let statements = sql(&[
        SchemaChange::RenameColumn {
            column: column("Mixed \"Case\""),
            name: "it's \"new\"".into(),
        },
        SchemaChange::ColumnType {
            column: column("Mixed \"Case\""),
            data_type: " varchar(40) ".into(),
        },
        SchemaChange::ColumnNullable {
            column: column("Mixed \"Case\""),
            nullable: false,
        },
        SchemaChange::ColumnDefault {
            column: column("Mixed \"Case\""),
            default: DefaultValue::Literal("it's".into()),
        },
        SchemaChange::ColumnComment {
            column: column("Mixed \"Case\""),
            comment: Some("Shown 'here'".into()),
        },
    ])?;
    assert_eq!(
        statements,
        vec![
            r#"ALTER TABLE "public"."users" ALTER COLUMN "Mixed ""Case""" TYPE varchar(40)"#,
            r#"ALTER TABLE "public"."users" ALTER COLUMN "Mixed ""Case""" SET NOT NULL"#,
            r#"ALTER TABLE "public"."users" ALTER COLUMN "Mixed ""Case""" SET DEFAULT 'it''s'"#,
            r#"COMMENT ON COLUMN "public"."users"."Mixed ""Case""" IS 'Shown ''here'''"#,
            r#"ALTER TABLE "public"."users" RENAME COLUMN "Mixed ""Case""" TO "it's ""new""""#,
        ]
    );
    Ok(())
}

#[test]
fn defaults_distinguish_expressions_literals_and_drop() -> Result<(), AlterError> {
    let statements = sql(&[
        SchemaChange::ColumnDefault {
            column: column("id"),
            default: DefaultValue::Expression("nextval('users_id_seq'::regclass)".into()),
        },
        SchemaChange::ColumnDefault {
            column: column("name"),
            default: DefaultValue::Literal("now()".into()),
        },
        SchemaChange::ColumnDefault {
            column: column("old"),
            default: DefaultValue::None,
        },
        SchemaChange::ColumnNullable {
            column: column("old"),
            nullable: true,
        },
        SchemaChange::ColumnComment {
            column: column("old"),
            comment: None,
        },
    ])?;
    assert_eq!(
        statements,
        vec![
            r#"ALTER TABLE "public"."users" ALTER COLUMN "id" SET DEFAULT (nextval('users_id_seq'::regclass))"#,
            r#"ALTER TABLE "public"."users" ALTER COLUMN "name" SET DEFAULT 'now()'"#,
            r#"ALTER TABLE "public"."users" ALTER COLUMN "old" DROP NOT NULL"#,
            r#"ALTER TABLE "public"."users" ALTER COLUMN "old" DROP DEFAULT"#,
            r#"COMMENT ON COLUMN "public"."users"."old" IS NULL"#,
        ]
    );
    Ok(())
}

#[test]
fn full_change_set_is_ordered_for_one_transaction() -> Result<(), AlterError> {
    let plan = plan(&[
        SchemaChange::RenameTable("people".into()),
        SchemaChange::CreateIndex(NewIndex {
            name: Some("people_email_idx".into()),
            columns: vec!["email".into(), "full_name".into()],
            unique: true,
            method: IndexMethod::Btree,
        }),
        SchemaChange::AddColumn(NewColumn {
            name: "email".into(),
            data_type: "text".into(),
            nullable: false,
            default: DefaultValue::Literal(String::new()),
            comment: Some("Login".into()),
        }),
        SchemaChange::RenameColumn {
            column: column("name"),
            name: "full_name".into(),
        },
        SchemaChange::DropColumn(column("old")),
        SchemaChange::DropIndex("users_name_idx".into()),
        SchemaChange::TableComment(Some("People".into())),
    ])?;
    assert_eq!(
        plan.statements,
        vec![
            r#"ALTER TABLE "public"."users" DROP COLUMN "old""#,
            r#"ALTER TABLE "public"."users" RENAME COLUMN "name" TO "full_name""#,
            r#"ALTER TABLE "public"."users" ADD COLUMN "email" text NOT NULL DEFAULT ''"#,
            r#"COMMENT ON COLUMN "public"."users"."email" IS 'Login'"#,
            r#"DROP INDEX "public"."users_name_idx""#,
            r#"CREATE UNIQUE INDEX "people_email_idx" ON "public"."users" USING btree ("email", "full_name")"#,
            r#"COMMENT ON TABLE "public"."users" IS 'People'"#,
            r#"ALTER TABLE "public"."users" RENAME TO "people""#,
        ]
    );
    assert!(plan.destructive);
    assert!(
        plan.warnings
            .iter()
            .any(|w| w.contains("permanently deletes"))
    );
    assert!(plan.warnings.iter().any(|w| w.contains("blocks writes")));
    Ok(())
}

#[test]
fn added_columns_and_unnamed_indexes() -> Result<(), AlterError> {
    let plan = plan(&[
        SchemaChange::AddColumn(NewColumn {
            name: "tags".into(),
            data_type: "text[]".into(),
            nullable: false,
            default: DefaultValue::None,
            comment: None,
        }),
        SchemaChange::AddColumn(NewColumn {
            name: "at".into(),
            data_type: "timestamptz".into(),
            nullable: true,
            default: DefaultValue::Expression("now()".into()),
            comment: None,
        }),
        SchemaChange::CreateIndex(NewIndex {
            name: Some(String::new()),
            columns: vec!["tags".into()],
            unique: false,
            method: IndexMethod::Gin,
        }),
    ])?;
    assert_eq!(
        plan.statements,
        vec![
            r#"ALTER TABLE "public"."users" ADD COLUMN "tags" text[] NOT NULL"#,
            r#"ALTER TABLE "public"."users" ADD COLUMN "at" timestamptz DEFAULT (now())"#,
            r#"CREATE INDEX ON "public"."users" USING gin ("tags")"#,
        ]
    );
    assert!(!plan.destructive);
    assert!(
        plan.warnings
            .iter()
            .any(|w| w.contains("without a default"))
    );
    Ok(())
}

#[test]
fn type_changes_are_destructive_and_validated() -> Result<(), AlterError> {
    let plan = plan(&[SchemaChange::ColumnType {
        column: column("id"),
        data_type: "bigint".into(),
    }])?;
    assert!(plan.destructive);
    for bad in ["int; DROP TABLE users", "text DEFAULT 'x'", "", "int -- x"] {
        assert_eq!(
            sql(&[SchemaChange::ColumnType {
                column: column("id"),
                data_type: bad.into(),
            }]),
            Err(AlterError::InvalidType(bad.into()))
        );
    }
    Ok(())
}

#[test]
fn unknown_conflicting_and_clashing_changes_are_rejected() {
    let unknown = sql(&[SchemaChange::DropColumn(column("ghost"))]);
    assert_eq!(unknown, Err(AlterError::UnknownColumn("ghost".into())));
    let twice = sql(&[
        SchemaChange::ColumnType {
            column: column("id"),
            data_type: "bigint".into(),
        },
        SchemaChange::ColumnType {
            column: column("id"),
            data_type: "text".into(),
        },
    ]);
    assert_eq!(twice, Err(AlterError::Conflict("id".into())));
    let dropped = sql(&[
        SchemaChange::DropColumn(column("old")),
        SchemaChange::ColumnComment {
            column: column("old"),
            comment: None,
        },
    ]);
    assert_eq!(dropped, Err(AlterError::Conflict("old".into())));
    let clash = sql(&[SchemaChange::RenameColumn {
        column: column("old"),
        name: "name".into(),
    }]);
    assert_eq!(clash, Err(AlterError::ColumnExists("name".into())));
    let added = sql(&[SchemaChange::AddColumn(NewColumn {
        name: "id".into(),
        data_type: "int".into(),
        nullable: true,
        default: DefaultValue::None,
        comment: None,
    })]);
    assert_eq!(added, Err(AlterError::ColumnExists("id".into())));
}

#[test]
fn renames_free_and_take_names_in_order() -> Result<(), AlterError> {
    // Dropping `old` first frees its name for a rename.
    let statements = sql(&[
        SchemaChange::DropColumn(column("old")),
        SchemaChange::RenameColumn {
            column: column("name"),
            name: "old".into(),
        },
    ])?;
    assert_eq!(statements.len(), 2);
    // An index on a renamed column must use the final name.
    let stale = sql(&[
        SchemaChange::RenameColumn {
            column: column("name"),
            name: "full_name".into(),
        },
        SchemaChange::CreateIndex(NewIndex {
            name: None,
            columns: vec!["name".into()],
            unique: false,
            method: IndexMethod::Btree,
        }),
    ]);
    assert_eq!(stale, Err(AlterError::UnknownColumn("name".into())));
    Ok(())
}

#[test]
fn indexes_must_exist_and_not_back_constraints() {
    assert_eq!(
        sql(&[SchemaChange::DropIndex("users_pkey".into())]),
        Err(AlterError::ConstraintIndex("users_pkey".into()))
    );
    assert_eq!(
        sql(&[SchemaChange::DropIndex("nope".into())]),
        Err(AlterError::UnknownIndex("nope".into()))
    );
    assert_eq!(
        sql(&[SchemaChange::CreateIndex(NewIndex {
            name: None,
            columns: Vec::new(),
            unique: false,
            method: IndexMethod::Hash,
        })]),
        Err(AlterError::NoIndexColumns)
    );
}

#[test]
fn names_defaults_and_relations_are_checked() {
    assert_eq!(sql(&[]), Err(AlterError::Empty));
    assert_eq!(
        sql(&[SchemaChange::RenameTable(String::new())]),
        Err(AlterError::InvalidName)
    );
    assert_eq!(
        sql(&[SchemaChange::RenameTable("a\0b".into())]),
        Err(AlterError::InvalidName)
    );
    assert_eq!(
        sql(&[SchemaChange::ColumnDefault {
            column: column("id"),
            default: DefaultValue::Expression(" ".into()),
        }]),
        Err(AlterError::InvalidDefault("id".into()))
    );
    assert_eq!(
        sql(&[SchemaChange::RenameColumn {
            column: column("id"),
            name: "id".into(),
        }]),
        Err(AlterError::Empty)
    );
    let view = TableDetail {
        info: Some(TableInfo {
            kind: RelationKind::View,
            ..TableInfo::default()
        }),
        ..detail()
    };
    let target = AlterTable {
        schema: "public",
        table: "v",
        detail: &view,
    };
    assert_eq!(
        alter_sql(&target, &[SchemaChange::TableComment(None)]),
        Err(AlterError::NotATable)
    );
}

#[test]
fn expressions_stay_inside_their_parentheses() -> Result<(), AlterError> {
    let statements = sql(&[SchemaChange::ColumnDefault {
        column: column("name"),
        default: DefaultValue::Expression("'a'); DROP TABLE users; --".into()),
    }])?;
    // The app rejects this because it no longer splits into one statement;
    // the generator never strips or rewrites the expression.
    assert_eq!(
        statements,
        vec![
            r#"ALTER TABLE "public"."users" ALTER COLUMN "name" SET DEFAULT ('a'); DROP TABLE users; --)"#
        ]
    );
    Ok(())
}
