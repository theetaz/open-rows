// SPDX-License-Identifier: Apache-2.0
use super::*;
use openrows_driver_api::schema::{
    ConstraintDetail, ConstraintKind, IndexDetail, TriggerDetail, TriggerEvent, TriggerTiming,
};

fn column(name: &str, data_type: &str, nullable: bool, default: Option<&str>) -> ColumnDetail {
    ColumnDetail {
        name: name.into(),
        data_type: data_type.into(),
        nullable,
        default: default.map(Into::into),
        ..ColumnDetail::default()
    }
}

fn users() -> TableDetail {
    TableDetail {
        columns: vec![
            ColumnDetail {
                generated: Generated::IdentityAlways,
                ..column("id", "bigint", false, None)
            },
            column("created_at", "timestamp(0) without time zone", false, Some("CURRENT_TIMESTAMP(0)")),
            ColumnDetail {
                comment: Some("Shown to other users".into()),
                ..column("name", "character varying(40)", true, Some("'anon'::character varying"))
            },
            ColumnDetail {
                generated: Generated::Stored,
                ..column("upper \"name\"", "text", true, Some("upper((name)::text)"))
            },
            column("org_id", "uuid", true, None),
        ],
        constraints: vec![
            ConstraintDetail {
                name: "users_pkey".into(),
                kind: ConstraintKind::Primary,
                definition: "PRIMARY KEY (id)".into(),
                columns: vec!["id".into()],
                references: None,
            },
            ConstraintDetail {
                name: "users_org_fk".into(),
                kind: ConstraintKind::Foreign,
                definition: "FOREIGN KEY (org_id) REFERENCES orgs(id) ON DELETE CASCADE".into(),
                columns: vec!["org_id".into()],
                references: None,
            },
        ],
        indexes: vec![
            IndexDetail {
                name: "users_pkey".into(),
                constraint: true,
                definition: "CREATE UNIQUE INDEX users_pkey ON public.users USING btree (id)".into(),
                ..IndexDetail::default()
            },
            IndexDetail {
                name: "users_name_idx".into(),
                definition: "CREATE INDEX users_name_idx ON public.users USING btree (lower((name)::text))".into(),
                ..IndexDetail::default()
            },
        ],
        triggers: vec![TriggerDetail {
            name: "touch".into(),
            timing: TriggerTiming::Before,
            events: vec![TriggerEvent::Update],
            row_level: true,
            function: "public.touch()".into(),
            enabled: true,
            definition: "CREATE TRIGGER touch BEFORE UPDATE ON public.users FOR EACH ROW EXECUTE FUNCTION touch()".into(),
        }],
        info: Some(TableInfo {
            row_estimate: Some(1788),
            comment: Some("People's accounts".into()),
            ..TableInfo::default()
        }),
    }
}

#[test]
fn tables_rebuild_columns_constraints_indexes_comments_and_triggers() -> Result<(), AlterError> {
    let sql = create_table_sql("public", "users", &users())?;
    let expected = r#"-- Table Definition
CREATE TABLE "public"."users" (
    "id" bigint GENERATED ALWAYS AS IDENTITY,
    "created_at" timestamp(0) without time zone NOT NULL DEFAULT CURRENT_TIMESTAMP(0),
    "name" character varying(40) DEFAULT 'anon'::character varying,
    "upper ""name""" text GENERATED ALWAYS AS (upper((name)::text)) STORED,
    "org_id" uuid,
    CONSTRAINT "users_pkey" PRIMARY KEY (id),
    CONSTRAINT "users_org_fk" FOREIGN KEY (org_id) REFERENCES orgs(id) ON DELETE CASCADE
);

CREATE INDEX users_name_idx ON public.users USING btree (lower((name)::text));

COMMENT ON TABLE "public"."users" IS 'People''s accounts';
COMMENT ON COLUMN "public"."users"."name" IS 'Shown to other users';

CREATE TRIGGER touch BEFORE UPDATE ON public.users FOR EACH ROW EXECUTE FUNCTION touch();
"#;
    assert_eq!(sql, expected);
    Ok(())
}

#[test]
fn constraint_backed_indexes_are_not_repeated() -> Result<(), AlterError> {
    let sql = create_table_sql("public", "users", &users())?;
    assert!(!sql.contains("CREATE UNIQUE INDEX users_pkey"));
    Ok(())
}

#[test]
fn empty_and_partitioned_tables() -> Result<(), AlterError> {
    let detail = TableDetail {
        info: Some(TableInfo {
            kind: RelationKind::PartitionedTable,
            partition_key: Some("RANGE (day)".into()),
            ..TableInfo::default()
        }),
        ..TableDetail::default()
    };
    assert_eq!(
        create_table_sql("s", "t", &detail)?,
        "-- Table Definition\nCREATE TABLE \"s\".\"t\" () PARTITION BY RANGE (day);\n"
    );
    Ok(())
}

#[test]
fn views_print_their_query() -> Result<(), AlterError> {
    let detail = TableDetail {
        columns: vec![column("n", "bigint", true, None)],
        info: Some(TableInfo {
            kind: RelationKind::MaterializedView,
            view_definition: Some(" SELECT count(*) AS n\n   FROM users;".into()),
            comment: Some("Totals".into()),
            ..TableInfo::default()
        }),
        ..TableDetail::default()
    };
    assert_eq!(
        create_table_sql("public", "totals", &detail)?,
        "-- Table Definition\nCREATE MATERIALIZED VIEW \"public\".\"totals\" AS\nSELECT count(*) AS n\n   FROM users;\n\nCOMMENT ON MATERIALIZED VIEW \"public\".\"totals\" IS 'Totals';\n"
    );
    Ok(())
}

#[test]
fn missing_relations_and_bad_names() -> Result<(), AlterError> {
    assert!(create_table_sql("s", "gone", &TableDetail::default())?.contains("was not found"));
    let detail = TableDetail {
        info: Some(TableInfo::default()),
        ..TableDetail::default()
    };
    assert!(create_table_sql("s", "", &detail).is_err());
    let comment = TableDetail {
        info: Some(TableInfo {
            comment: Some(r"C:\path".into()),
            ..TableInfo::default()
        }),
        ..TableDetail::default()
    };
    assert!(create_table_sql("s", "t", &comment)?.contains(r"IS E'C:\\path';"));
    Ok(())
}
