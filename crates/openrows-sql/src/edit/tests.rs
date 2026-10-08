// SPDX-License-Identifier: Apache-2.0
use super::*;
use crate::Dialect;
fn table() -> EditTable {
    EditTable {
        table: TableRef {
            schema: "public".into(),
            name: "a\"b".into(),
        },
        columns: vec!["id".into(), "name".into(), "computed".into()],
        key: vec!["id".into()],
        protected: vec!["computed".into()],
    }
}
fn edit() -> Edit {
    Edit {
        kind: EditKind::Update,
        original: Values::from([
            ("id".into(), Some("1".into())),
            ("name".into(), None),
            ("computed".into(), Some("2".into())),
        ]),
        values: Changes::from([(
            "name".into(),
            CellValue::Text("'; DELETE FROM x; --".into()),
        )]),
    }
}
#[test]
fn values_are_bound_and_complete_snapshot_guards_conflicts() -> Result<(), SelectError> {
    let bound = edit_sql(Dialect::Postgres, &table(), &edit())?;
    assert!(
        bound
            .sql
            .starts_with("UPDATE \"public\".\"a\"\"b\" SET \"name\" = $1 WHERE")
    );
    assert!(!bound.sql.contains("DELETE"));
    assert!(
        bound
            .sql
            .contains("\"name\"::text IS NOT DISTINCT FROM $4::text")
    );
    assert_eq!(bound.params[0].as_deref(), Some("'; DELETE FROM x; --"));
    assert_eq!(bound.params[3], None);
    Ok(())
}
#[test]
fn missing_keys_snapshots_protected_and_unknown_columns_are_refused() {
    let mut t = table();
    let mut e = edit();
    t.key.clear();
    assert!(edit_sql(Dialect::Postgres, &t, &e).is_err());
    t = table();
    e.original.remove("id");
    assert!(edit_sql(Dialect::Postgres, &t, &e).is_err());
    e = edit();
    e.values.insert("computed".into(), CellValue::Null);
    assert!(edit_sql(Dialect::Postgres, &t, &e).is_err());
    e = edit();
    e.values.insert("stranger".into(), CellValue::Null);
    assert!(edit_sql(Dialect::Postgres, &t, &e).is_err());
    e = edit();
    e.values.insert("name".into(), CellValue::Text("\0".into()));
    assert!(edit_sql(Dialect::Postgres, &t, &e).is_err());
}
#[test]
fn insert_defaults_and_delete_are_parameterised() -> Result<(), SelectError> {
    let mut e = Edit {
        kind: EditKind::Insert,
        original: Values::new(),
        values: Changes::new(),
    };
    assert!(
        edit_sql(Dialect::Postgres, &table(), &e)?
            .sql
            .ends_with("DEFAULT VALUES")
    );
    e = edit();
    e.kind = EditKind::Delete;
    e.values.clear();
    assert!(
        edit_sql(Dialect::Postgres, &table(), &e)?
            .sql
            .starts_with("DELETE FROM")
    );
    Ok(())
}
fn update(values: Changes) -> Result<BoundSql, SelectError> {
    edit_sql(Dialect::Postgres, &table(), &Edit { values, ..edit() })
}
fn insert(values: Changes) -> Result<BoundSql, SelectError> {
    edit_sql(
        Dialect::Postgres,
        &table(),
        &Edit {
            kind: EditKind::Insert,
            original: Values::new(),
            values,
        },
    )
}
#[test]
fn default_updates_use_the_keyword_and_never_bind_it() -> Result<(), SelectError> {
    let bound = update(Changes::from([("name".into(), CellValue::Default)]))?;
    assert!(
        bound
            .sql
            .starts_with("UPDATE \"public\".\"a\"\"b\" SET \"name\" = DEFAULT WHERE")
    );
    assert!(bound.sql.contains("IS NOT DISTINCT FROM $1::text"));
    assert!(!bound.sql.contains("$4"));
    assert_eq!(bound.params.len(), 3, "only the guard is bound");
    assert!(!bound.params.contains(&Some("DEFAULT".into())));
    Ok(())
}
#[test]
fn the_text_default_is_ordinary_bound_data() -> Result<(), SelectError> {
    let bound = update(Changes::from([(
        "name".into(),
        CellValue::Text("DEFAULT".into()),
    )]))?;
    assert!(bound.sql.contains("SET \"name\" = $1 WHERE"));
    assert_eq!(bound.params[0].as_deref(), Some("DEFAULT"));
    Ok(())
}
#[test]
fn null_empty_and_text_bind_distinct_parameters() -> Result<(), SelectError> {
    let mut t = table();
    t.columns.push("note".into());
    let mut e = edit();
    e.original.insert("note".into(), Some("old".into()));
    e.values = Changes::from([
        ("name".into(), CellValue::Null),
        ("note".into(), CellValue::Text(String::new())),
    ]);
    let bound = edit_sql(Dialect::Postgres, &t, &e)?;
    assert!(bound.sql.contains("SET \"name\" = $1, \"note\" = $2 WHERE"));
    assert_eq!(bound.params[0], None);
    assert_eq!(bound.params[1].as_deref(), Some(""));
    let text = update(Changes::from([(
        "name".into(),
        CellValue::Text("NULL".into()),
    )]))?;
    assert_eq!(text.params[0].as_deref(), Some("NULL"));
    Ok(())
}
#[test]
fn inserts_omit_default_columns() -> Result<(), SelectError> {
    let bound = insert(Changes::from([
        ("id".into(), CellValue::Text("9".into())),
        ("name".into(), CellValue::Default),
    ]))?;
    assert_eq!(
        bound.sql,
        "INSERT INTO \"public\".\"a\"\"b\" (\"id\") VALUES ($1)"
    );
    assert_eq!(bound.params, vec![Some("9".into())]);
    let all = insert(Changes::from([("name".into(), CellValue::Default)]))?;
    assert!(all.sql.ends_with("DEFAULT VALUES"));
    assert!(all.params.is_empty());
    let null = insert(Changes::from([("name".into(), CellValue::Null)]))?;
    assert!(null.sql.ends_with("(\"name\") VALUES ($1)"));
    assert_eq!(null.params, vec![None]);
    Ok(())
}
#[test]
fn file_bytea_hex_is_bound_verbatim() -> Result<(), SelectError> {
    let hex = "\\x00ff7f0a";
    let bound = update(Changes::from([(
        "name".into(),
        CellValue::Text(hex.into()),
    )]))?;
    assert_eq!(bound.params[0].as_deref(), Some(hex));
    assert!(!bound.sql.contains("00ff"));
    Ok(())
}
#[test]
fn protected_columns_refuse_default_too() {
    let mut e = edit();
    e.values.insert("computed".into(), CellValue::Default);
    assert!(edit_sql(Dialect::Postgres, &table(), &e).is_err());
}
