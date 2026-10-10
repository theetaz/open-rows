// SPDX-License-Identifier: Apache-2.0
//! Table-browse and row-edit SQL for `MySQL` and `SQLite`: quoting,
//! placeholders, text guards and dialect gaps (data-safety critical).

use crate::edit::{CellValue, Changes, Edit, EditKind, EditTable, Values, edit_sql};
use crate::select::{
    Combinator, Filter, FilterOp, Page, SelectError, Sort, SortDirection, TableQuery, TableRef,
    count_sql, page_sql,
};
use crate::{Dialect, StatementKind, classify};

fn known() -> Vec<String> {
    ["id", "name", "we`ird"].map(str::to_owned).to_vec()
}

fn query(filters: Vec<Filter>) -> TableQuery {
    TableQuery {
        table: TableRef {
            schema: "shop".into(),
            name: "or`ders".into(),
        },
        columns: Vec::new(),
        filters,
        groups: Vec::new(),
        combinator: Combinator::And,
        sort: Some(Sort {
            column: "we`ird".into(),
            direction: SortDirection::Descending,
        }),
        row_id: None,
    }
}

fn filter(column: &str, op: FilterOp, value: &str) -> Filter {
    Filter {
        column: column.into(),
        op,
        value: value.into(),
        second: String::new(),
    }
}

const PAGE: Page = Page {
    limit: 100,
    offset: 200,
};

#[test]
fn mysql_pages_use_backticks_and_anonymous_placeholders() -> Result<(), SelectError> {
    let filters = vec![
        filter("id", FilterOp::Greater, "5"),
        filter("name", FilterOp::Contains, "50%_off"),
        filter("name", FilterOp::Regex, "^a"),
        filter("id", FilterOp::In, "1, 2"),
    ];
    let bound = page_sql(Dialect::MySql, &query(filters), &known(), PAGE)?;
    assert_eq!(
        bound.sql,
        "SELECT * FROM `shop`.`or``ders` WHERE (`id` > ?) AND \
         (CAST(`name` AS CHAR) LIKE ?) AND (CAST(`name` AS CHAR) REGEXP ?) AND \
         (`id` IN (?, ?)) ORDER BY `we``ird` DESC LIMIT 100 OFFSET 200"
    );
    let params: Vec<_> = bound.params.iter().flatten().map(String::as_str).collect();
    assert_eq!(params, ["5", "%50\\%\\_off%", "^a", "1", "2"]);
    let class = classify(&bound.sql, Dialect::MySql);
    assert_eq!(class.kind, StatementKind::Read);
    assert!(!class.dangerous);
    Ok(())
}

#[test]
fn sqlite_pages_number_placeholders_and_escape_like() -> Result<(), SelectError> {
    let filters = vec![
        filter("name", FilterOp::StartsWith, "a"),
        filter("id", FilterOp::IsEmpty, ""),
    ];
    let bound = page_sql(Dialect::Sqlite, &query(filters), &known(), PAGE)?;
    assert_eq!(
        bound.sql,
        "SELECT * FROM \"shop\".\"or`ders\" WHERE \
         (CAST(\"name\" AS TEXT) LIKE ?1 ESCAPE '\\') AND (CAST(\"id\" AS TEXT) = '') \
         ORDER BY \"we`ird\" DESC LIMIT 100 OFFSET 200"
    );
    assert_eq!(
        classify(&bound.sql, Dialect::Sqlite).kind,
        StatementKind::Read
    );
    let count = count_sql(Dialect::Sqlite, &query(Vec::new()), &known())?;
    assert_eq!(count.sql, "SELECT count(*) FROM \"shop\".\"or`ders\"");
    Ok(())
}

#[test]
fn sqlite_has_no_regex_filter_and_unknown_columns_are_refused() {
    let regex = vec![filter("name", FilterOp::Regex, "a")];
    assert!(matches!(
        page_sql(Dialect::Sqlite, &query(regex), &known(), PAGE),
        Err(SelectError::Unsupported(_))
    ));
    let injected = vec![filter("name` = 1 OR `1", FilterOp::Equal, "x")];
    assert!(matches!(
        page_sql(Dialect::MySql, &query(injected), &known(), PAGE),
        Err(SelectError::UnknownColumn(_))
    ));
}

fn table() -> EditTable {
    EditTable {
        table: TableRef {
            schema: "shop".into(),
            name: "users".into(),
        },
        columns: vec!["id".into(), "name".into()],
        key: vec!["id".into()],
        protected: Vec::new(),
    }
}

fn original() -> Values {
    Values::from([("id".into(), Some("1".into())), ("name".into(), None)])
}

fn edit(kind: EditKind, values: Changes) -> Edit {
    let original = if kind == EditKind::Insert {
        Values::new()
    } else {
        original()
    };
    Edit {
        kind,
        original,
        values,
    }
}

fn text(value: &str) -> Changes {
    Changes::from([("name".into(), CellValue::Text(value.into()))])
}

#[test]
fn mysql_edits_guard_every_original_column_byte_exactly() -> Result<(), SelectError> {
    let update = edit_sql(
        Dialect::MySql,
        &table(),
        &edit(EditKind::Update, text("'; DROP TABLE x; --")),
    )?;
    assert_eq!(
        update.sql,
        "UPDATE `shop`.`users` SET `name` = ? WHERE \
         CAST(`id` AS CHAR) COLLATE utf8mb4_bin <=> ? AND \
         CAST(`name` AS CHAR) COLLATE utf8mb4_bin <=> ?"
    );
    assert_eq!(
        update.params,
        vec![Some("'; DROP TABLE x; --".into()), Some("1".into()), None]
    );
    let delete = edit_sql(
        Dialect::MySql,
        &table(),
        &edit(EditKind::Delete, Changes::new()),
    )?;
    assert!(
        delete
            .sql
            .starts_with("DELETE FROM `shop`.`users` WHERE CAST(`id`")
    );
    for sql in [&update.sql, &delete.sql] {
        let class = classify(sql, Dialect::MySql);
        assert_eq!(class.kind, StatementKind::Write);
        assert!(!class.dangerous, "bounded DML is never dangerous: {sql}");
    }
    let defaults = Changes::from([("name".into(), CellValue::Default)]);
    let insert = edit_sql(Dialect::MySql, &table(), &edit(EditKind::Insert, defaults))?;
    assert_eq!(insert.sql, "INSERT INTO `shop`.`users` () VALUES ()");
    Ok(())
}

#[test]
fn sqlite_edits_number_placeholders_and_refuse_update_defaults() -> Result<(), SelectError> {
    let update = edit_sql(
        Dialect::Sqlite,
        &table(),
        &edit(EditKind::Update, text("x")),
    )?;
    assert_eq!(
        update.sql,
        "UPDATE \"shop\".\"users\" SET \"name\" = ?1 WHERE \
         CAST(\"id\" AS TEXT) COLLATE BINARY IS ?2 AND \
         CAST(\"name\" AS TEXT) COLLATE BINARY IS ?3"
    );
    let defaults = Changes::from([("name".into(), CellValue::Default)]);
    assert!(matches!(
        edit_sql(
            Dialect::Sqlite,
            &table(),
            &edit(EditKind::Update, defaults.clone())
        ),
        Err(SelectError::Unsupported(_))
    ));
    let insert = edit_sql(Dialect::Sqlite, &table(), &edit(EditKind::Insert, defaults))?;
    assert_eq!(insert.sql, "INSERT INTO \"shop\".\"users\" DEFAULT VALUES");
    let class = classify(&update.sql, Dialect::Sqlite);
    assert!(class.kind == StatementKind::Write && !class.dangerous);
    Ok(())
}

#[test]
fn keyless_edits_never_produce_sql() {
    let mut keyless = table();
    keyless.key.clear();
    for dialect in [Dialect::MySql, Dialect::Sqlite] {
        assert!(edit_sql(dialect, &keyless, &edit(EditKind::Delete, Changes::new())).is_err());
        let mut partial = edit(EditKind::Delete, Changes::new());
        partial.original.remove("name");
        assert!(edit_sql(dialect, &table(), &partial).is_err());
    }
}
