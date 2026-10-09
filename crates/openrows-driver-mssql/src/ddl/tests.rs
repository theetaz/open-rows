// SPDX-License-Identifier: Apache-2.0
use openrows_driver_api::schema::{
    ColumnDetail, ConstraintDetail, ConstraintKind, ForeignTarget, Identity, IndexDetail,
    IndexMethod, NewCheck, NewForeignKey, RelationKind, TableDetail, TableInfo,
};
use openrows_sql::{Dialect, StatementKind, classify, split};

const DIALECT: Dialect = Dialect::SqlServer;

use super::definition::{column_clauses, column_line, create_table};
use super::*;
use crate::catalog::rows::ColumnRow;

fn row(name: &str, data_type: &str, nullable: bool) -> ColumnRow {
    ColumnRow {
        name: name.to_owned(),
        data_type: data_type.to_owned(),
        base_type: data_type.split('(').next().unwrap_or_default().to_owned(),
        nullable,
        ..ColumnRow::default()
    }
}

fn detail() -> TableDetail {
    let mut name = row("name", "varchar(40)", false);
    name.collation = Some("Latin1_General_BIN".to_owned());
    let mut qty = row("qty", "int", true);
    qty.default = Some("((5))".to_owned());
    qty.default_name = Some("df_t_q]ty".to_owned());
    qty.comment = Some("count".to_owned());
    let rows = [row("id", "int", false), name, qty];
    let relation = Relation {
        schema: "dbo",
        table: "t",
    };
    TableDetail {
        columns: rows
            .iter()
            .map(|row| ColumnDetail {
                name: row.name.clone(),
                data_type: row.data_type.clone(),
                nullable: row.nullable,
                default: row.default.clone(),
                comment: row.comment.clone(),
                ..ColumnDetail::default()
            })
            .collect(),
        indexes: vec![IndexDetail {
            name: "ix_old".to_owned(),
            ..IndexDetail::default()
        }],
        info: Some(TableInfo {
            kind: RelationKind::Table,
            definition: Some(create_table(&relation, &rows, &[], &[])),
            ..TableInfo::default()
        }),
        constraints: vec![ConstraintDetail {
            name: "ck_old".to_owned(),
            kind: ConstraintKind::Check,
            definition: "CHECK ([qty]>(0))".to_owned(),
            columns: vec!["qty".to_owned()],
            references: None,
        }],
        ..TableDetail::default()
    }
}

fn plan(changes: &[SchemaChange]) -> Result<AlterPlan, AlterError> {
    let detail = detail();
    alter_sql(
        &AlterTable {
            schema: "dbo",
            table: "t",
            detail: &detail,
        },
        changes,
    )
}

#[test]
fn definitions_round_trip_default_names_and_collations() {
    let detail = detail();
    let definition = detail
        .info
        .and_then(|info| info.definition)
        .unwrap_or_default();
    assert!(definition.contains("[qty] int NULL CONSTRAINT [df_t_q]]ty] DEFAULT ((5))"));
    let clauses = column_clauses(&definition, "qty").unwrap_or_default();
    assert_eq!(clauses.default_constraint.as_deref(), Some("df_t_q]ty"));
    let name = column_clauses(&definition, "name").unwrap_or_default();
    assert_eq!(name.collation.as_deref(), Some("Latin1_General_BIN"));
    let mut computed = row("total", "int", true);
    computed.computed = Some("([a]*(2))".to_owned());
    assert_eq!(column_line(&computed), "[total] AS ([a]*(2))");
}

#[test]
fn column_changes_restate_type_keep_collation_and_replace_defaults() -> Result<(), AlterError> {
    let plan = plan(&[
        SchemaChange::ColumnNullable {
            column: "name".into(),
            nullable: true,
        },
        SchemaChange::ColumnDefault {
            column: "qty".into(),
            default: DefaultValue::Expression("1 + 1".into()),
        },
        SchemaChange::ColumnComment {
            column: "qty".into(),
            comment: None,
        },
        SchemaChange::RenameColumn {
            column: "qty".into(),
            name: "amount".into(),
        },
    ])?;
    assert_eq!(
        plan.statements,
        [
            "ALTER TABLE [dbo].[t] ALTER COLUMN [name] varchar(40) COLLATE Latin1_General_BIN NULL",
            "ALTER TABLE [dbo].[t] DROP CONSTRAINT [df_t_q]]ty]",
            "ALTER TABLE [dbo].[t] ADD CONSTRAINT [DF_t_qty] DEFAULT (1 + 1) FOR [qty]",
            "EXEC sys.sp_dropextendedproperty @name = N'MS_Description', @level0type = N'SCHEMA', \
             @level0name = N'dbo', @level1type = N'TABLE', @level1name = N't', \
             @level2type = N'COLUMN', @level2name = N'qty'",
            "EXEC sys.sp_rename N'[dbo].[t].[qty]', N'amount', N'COLUMN'",
        ]
    );
    assert!(!plan.destructive);
    every_statement_is_one_ddl(&plan);
    Ok(())
}

#[test]
fn adds_drops_indexes_and_table_options() -> Result<(), AlterError> {
    let plan = plan(&[
        SchemaChange::DropColumn("qty".into()),
        SchemaChange::AddColumn(NewColumn {
            name: "flag".into(),
            data_type: "bit".into(),
            nullable: false,
            default: DefaultValue::Literal("0".into()),
            comment: Some("on?".into()),
            ..NewColumn::default()
        }),
        SchemaChange::DropIndex("ix_old".into()),
        SchemaChange::CreateIndex(NewIndex {
            name: None,
            columns: vec!["name".into(), "flag".into()],
            unique: true,
            method: IndexMethod::Btree,
            ..NewIndex::default()
        }),
        SchemaChange::TableComment(Some("things".into())),
        SchemaChange::RenameTable("things".into()),
    ])?;
    assert_eq!(plan.statements[0], "DROP INDEX [ix_old] ON [dbo].[t]");
    assert_eq!(
        plan.statements[1],
        "ALTER TABLE [dbo].[t] DROP CONSTRAINT [df_t_q]]ty]"
    );
    assert_eq!(
        plan.statements[2],
        "ALTER TABLE [dbo].[t] DROP COLUMN [qty]"
    );
    assert_eq!(
        plan.statements[3],
        "ALTER TABLE [dbo].[t] ADD [flag] bit NOT NULL CONSTRAINT [DF_t_flag] DEFAULT (N'0') WITH VALUES"
    );
    assert!(plan.statements[4].starts_with("EXEC sys.sp_addextendedproperty"));
    assert_eq!(
        plan.statements[5],
        "CREATE UNIQUE INDEX [IX_t_name_flag] ON [dbo].[t] ([name], [flag])"
    );
    assert!(plan.statements[6].contains("sp_addextendedproperty"));
    assert_eq!(
        plan.statements[7],
        "EXEC sys.sp_rename N'[dbo].[t]', N'things'"
    );
    assert!(plan.destructive);
    every_statement_is_one_ddl(&plan);
    Ok(())
}

#[test]
fn invalid_change_sets_are_refused() {
    let bad_type = plan(&[SchemaChange::ColumnType {
        column: "qty".into(),
        data_type: "int; DROP TABLE t".into(),
    }]);
    assert!(matches!(bad_type, Err(AlterError::InvalidType(_))));
    let bad_default = plan(&[SchemaChange::ColumnDefault {
        column: "qty".into(),
        default: DefaultValue::Expression("1; DROP TABLE t".into()),
    }]);
    assert!(matches!(bad_default, Err(AlterError::InvalidDefault(_))));
    let hash = plan(&[SchemaChange::CreateIndex(NewIndex {
        name: None,
        columns: vec!["id".into()],
        unique: false,
        method: IndexMethod::Gin,
        ..NewIndex::default()
    })]);
    assert!(matches!(hash, Err(AlterError::Unsupported(_))));
    assert!(matches!(plan(&[]), Err(AlterError::Empty)));
    let missing = TableDetail::default();
    assert_eq!(
        table_ddl(("dbo", "x"), &missing),
        "-- dbo.x was not found\n"
    );
}

#[test]
fn constraints_collations_and_identity_follow_t_sql() -> Result<(), AlterError> {
    let plan = plan(&[
        SchemaChange::ColumnCollation {
            column: "name".into(),
            charset: None,
            collation: Some("Latin1_General_100_CI_AS".into()),
        },
        SchemaChange::AddForeignKey(NewForeignKey {
            columns: vec!["qty".into()],
            target: ForeignTarget {
                schema: "dbo".into(),
                table: "q".into(),
                columns: vec!["id".into()],
            },
            ..NewForeignKey::default()
        }),
        SchemaChange::AddCheck(NewCheck {
            name: Some("ck_qty".into()),
            expression: "qty > 0".into(),
        }),
        SchemaChange::DropConstraint("ck_old".into()),
    ])?;
    assert_eq!(
        plan.statements,
        [
            "ALTER TABLE [dbo].[t] DROP CONSTRAINT [ck_old]",
            "ALTER TABLE [dbo].[t] ALTER COLUMN [name] varchar(40) COLLATE Latin1_General_100_CI_AS NOT NULL",
            "ALTER TABLE [dbo].[t] ADD FOREIGN KEY ([qty]) REFERENCES [dbo].[q] ([id]) \
             ON DELETE NO ACTION ON UPDATE NO ACTION",
            "ALTER TABLE [dbo].[t] ADD CONSTRAINT [ck_qty] CHECK (qty > 0)",
        ]
    );
    every_statement_is_one_ddl(&plan);
    let identity = alter_sql(
        &AlterTable {
            schema: "dbo",
            table: "t",
            detail: &detail(),
        },
        &[SchemaChange::ColumnIdentity {
            column: "qty".into(),
            identity: Identity::ByDefault,
        }],
    );
    assert!(matches!(identity, Err(AlterError::Unsupported(_))));
    let charset = alter_sql(
        &AlterTable {
            schema: "dbo",
            table: "t",
            detail: &detail(),
        },
        &[SchemaChange::ColumnCollation {
            column: "name".into(),
            charset: Some("utf8mb4".into()),
            collation: None,
        }],
    );
    assert!(matches!(charset, Err(AlterError::Unsupported(_))));
    Ok(())
}

/// The app runs only single statements that classify as DDL.
fn every_statement_is_one_ddl(plan: &AlterPlan) {
    for sql in &plan.statements {
        assert_eq!(split(sql, DIALECT).len(), 1, "{sql}");
        assert_eq!(classify(sql, DIALECT).kind, StatementKind::Ddl, "{sql}");
    }
}
