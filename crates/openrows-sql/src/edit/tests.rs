// SPDX-License-Identifier: Apache-2.0
use super::*;
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
        values: Values::from([("name".into(), Some("'; DELETE FROM x; --".into()))]),
    }
}
#[test]
fn values_are_bound_and_complete_snapshot_guards_conflicts() -> Result<(), SelectError> {
    let bound = edit_sql(&table(), &edit())?;
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
    assert!(edit_sql(&t, &e).is_err());
    t = table();
    e.original.remove("id");
    assert!(edit_sql(&t, &e).is_err());
    e = edit();
    e.values.insert("computed".into(), None);
    assert!(edit_sql(&t, &e).is_err());
    e = edit();
    e.values.insert("stranger".into(), None);
    assert!(edit_sql(&t, &e).is_err());
    e = edit();
    e.values.insert("name".into(), Some("\0".into()));
    assert!(edit_sql(&t, &e).is_err());
}
#[test]
fn insert_defaults_and_delete_are_parameterised() -> Result<(), SelectError> {
    let mut e = Edit {
        kind: EditKind::Insert,
        original: Values::new(),
        values: Values::new(),
    };
    assert!(edit_sql(&table(), &e)?.sql.ends_with("DEFAULT VALUES"));
    e = edit();
    e.kind = EditKind::Delete;
    e.values.clear();
    assert!(edit_sql(&table(), &e)?.sql.starts_with("DELETE FROM"));
    Ok(())
}
