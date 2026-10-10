// SPDX-License-Identifier: Apache-2.0
//! A keyless `SQLite` table's rowid: selected as the trailing hidden column,
//! guarded on by edits and never written (data-safety critical).
use crate::edit::{CellValue, Changes, Edit, EditKind, EditTable, Values, edit_sql};
use crate::select::{Combinator, Page, SelectError, TableQuery, TableRef, count_sql, page_sql};
use crate::{Dialect, StatementKind, classify};

const PAGE: Page = Page {
    limit: 300,
    offset: 0,
};

fn known() -> Vec<String> {
    ["a", "b"].map(str::to_owned).to_vec()
}

fn query(columns: &[&str], row_id: Option<&str>) -> TableQuery {
    TableQuery {
        table: TableRef {
            schema: "main".into(),
            name: "loose".into(),
        },
        columns: columns.iter().map(|name| (*name).to_owned()).collect(),
        filters: Vec::new(),
        groups: Vec::new(),
        combinator: Combinator::And,
        sort: None,
        row_id: row_id.map(str::to_owned),
    }
}

#[test]
fn the_rowid_is_the_last_projected_column() -> Result<(), SelectError> {
    let all = page_sql(Dialect::Sqlite, &query(&[], Some("rowid")), &known(), PAGE)?;
    assert_eq!(
        all.sql,
        "SELECT *, \"rowid\" AS \"rowid\" FROM \"main\".\"loose\" LIMIT 300 OFFSET 0"
    );
    let listed = page_sql(
        Dialect::Sqlite,
        &query(&["b"], Some("_rowid_")),
        &known(),
        PAGE,
    )?;
    assert_eq!(
        listed.sql,
        "SELECT \"b\", \"_rowid_\" AS \"_rowid_\" FROM \"main\".\"loose\" LIMIT 300 OFFSET 0"
    );
    let oid = page_sql(Dialect::Sqlite, &query(&[], Some("oid")), &known(), PAGE)?;
    assert!(oid.sql.starts_with("SELECT *, \"oid\" AS \"oid\" FROM"));
    assert_eq!(
        classify(&all.sql, Dialect::Sqlite).kind,
        StatementKind::Read
    );
    Ok(())
}

#[test]
fn counts_ignore_the_rowid() -> Result<(), SelectError> {
    let with = count_sql(Dialect::Sqlite, &query(&[], Some("rowid")), &known())?;
    let without = count_sql(Dialect::Sqlite, &query(&[], None), &known())?;
    assert_eq!(with, without);
    assert_eq!(with.sql, "SELECT count(*) FROM \"main\".\"loose\"");
    Ok(())
}

#[test]
fn only_sqlite_aliases_that_no_column_shadows_are_selected() {
    for dialect in [Dialect::Postgres, Dialect::MySql] {
        assert!(matches!(
            page_sql(dialect, &query(&[], Some("rowid")), &known(), PAGE),
            Err(SelectError::Unsupported(_))
        ));
    }
    let shadowing = vec!["a".to_owned(), "RowID".to_owned()];
    assert_eq!(
        page_sql(
            Dialect::Sqlite,
            &query(&[], Some("rowid")),
            &shadowing,
            PAGE
        ),
        Err(SelectError::UnknownColumn("rowid".into()))
    );
    for name in ["ctid", "a", "rowid\" FROM x; --", "ROWID", ""] {
        assert_eq!(
            page_sql(Dialect::Sqlite, &query(&[], Some(name)), &known(), PAGE),
            Err(SelectError::UnknownColumn(name.into())),
            "{name:?}"
        );
    }
}

fn keyless() -> EditTable {
    EditTable {
        table: TableRef {
            schema: "main".into(),
            name: "loose".into(),
        },
        columns: vec!["a".into(), "b".into(), "rowid".into()],
        key: vec!["rowid".into()],
        protected: vec!["rowid".into()],
    }
}

fn original() -> Values {
    Values::from([
        ("a".into(), Some("x".into())),
        ("b".into(), None),
        ("rowid".into(), Some("7".into())),
    ])
}

#[test]
fn edits_guard_on_the_rowid_and_stay_bounded() -> Result<(), SelectError> {
    let update = Edit {
        kind: EditKind::Update,
        original: original(),
        values: Changes::from([("a".into(), CellValue::Text("y".into()))]),
    };
    let bound = edit_sql(Dialect::Sqlite, &keyless(), &update)?;
    assert_eq!(
        bound.sql,
        "UPDATE \"main\".\"loose\" SET \"a\" = ?1 WHERE \
         CAST(\"a\" AS TEXT) COLLATE BINARY IS ?2 AND \
         CAST(\"b\" AS TEXT) COLLATE BINARY IS ?3 AND \
         CAST(\"rowid\" AS TEXT) COLLATE BINARY IS ?4"
    );
    assert_eq!(bound.params.last(), Some(&Some("7".to_owned())));
    let delete = Edit {
        kind: EditKind::Delete,
        original: original(),
        values: Changes::new(),
    };
    let bound = edit_sql(Dialect::Sqlite, &keyless(), &delete)?;
    assert!(
        bound
            .sql
            .ends_with("CAST(\"rowid\" AS TEXT) COLLATE BINARY IS ?3")
    );
    assert!(!classify(&bound.sql, Dialect::Sqlite).dangerous);
    Ok(())
}

#[test]
fn the_rowid_is_never_written_and_always_required() {
    let mut update = Edit {
        kind: EditKind::Update,
        original: original(),
        values: Changes::from([("rowid".into(), CellValue::Text("8".into()))]),
    };
    assert_eq!(
        edit_sql(Dialect::Sqlite, &keyless(), &update).map(|bound| bound.sql),
        Err(SelectError::UnknownColumn("rowid".into()))
    );
    let insert = Edit {
        kind: EditKind::Insert,
        original: Values::new(),
        values: Changes::from([("rowid".into(), CellValue::Text("8".into()))]),
    };
    assert!(edit_sql(Dialect::Sqlite, &keyless(), &insert).is_err());
    update.values = Changes::from([("a".into(), CellValue::Null)]);
    update.original.insert("rowid".into(), None);
    assert_eq!(
        edit_sql(Dialect::Sqlite, &keyless(), &update).map(|bound| bound.sql),
        Err(SelectError::NoColumns),
        "a NULL rowid identifies nothing"
    );
    update.original.remove("rowid");
    assert!(edit_sql(Dialect::Sqlite, &keyless(), &update).is_err());
}
