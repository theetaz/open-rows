// SPDX-License-Identifier: Apache-2.0
//! Filter-builder SQL per dialect (FR-GRID-07/11): raw predicates beside
//! bound filters, AND/OR, foreign-key equality filters and the lexical
//! checks that keep a raw predicate one expression.
use crate::select::{
    Combinator, Filter, FilterOp, Page, SelectError, TableQuery, TableRef, count_sql, page_sql,
};
use crate::{Dialect, StatementKind, classify};

const PAGE: Page = Page {
    limit: 50,
    offset: 0,
};

fn known() -> Vec<String> {
    ["id", "name", "org_id", "region"]
        .map(str::to_owned)
        .to_vec()
}

fn filter(column: &str, op: FilterOp, value: &str) -> Filter {
    Filter {
        column: column.to_owned(),
        op,
        value: value.to_owned(),
        second: String::new(),
    }
}

fn raw(text: &str) -> Filter {
    filter("", FilterOp::Raw, text)
}

fn query(filters: Vec<Filter>, combinator: Combinator) -> TableQuery {
    TableQuery {
        table: TableRef {
            schema: "app".to_owned(),
            name: "users".to_owned(),
        },
        columns: Vec::new(),
        filters,
        groups: Vec::new(),
        combinator,
        sort: None,
        row_id: None,
    }
}

fn page(
    dialect: Dialect,
    filters: Vec<Filter>,
    combinator: Combinator,
) -> Result<String, SelectError> {
    page_sql(dialect, &query(filters, combinator), &known(), PAGE).map(|bound| bound.sql)
}

#[test]
fn raw_predicates_join_bound_filters_in_every_dialect() -> Result<(), SelectError> {
    let filters = || {
        vec![
            filter("id", FilterOp::Greater, "5"),
            raw("  lower(name) <> 'x' AND (org_id IS NULL OR org_id > 2) "),
            filter("region", FilterOp::Equal, "eu"),
        ]
    };
    let pg = page_sql(
        Dialect::Postgres,
        &query(filters(), Combinator::Or),
        &known(),
        PAGE,
    )?;
    assert_eq!(
        pg.sql,
        "SELECT * FROM \"app\".\"users\" WHERE (\"id\" > $1) OR \
         (lower(name) <> 'x' AND (org_id IS NULL OR org_id > 2)) OR \
         (\"region\" = $2) LIMIT 50 OFFSET 0"
    );
    assert_eq!(pg.params, vec![Some("5".to_owned()), Some("eu".to_owned())]);
    assert_eq!(
        page(Dialect::MySql, filters(), Combinator::And)?,
        "SELECT * FROM `app`.`users` WHERE (`id` > ?) AND \
         (lower(name) <> 'x' AND (org_id IS NULL OR org_id > 2)) AND \
         (`region` = ?) LIMIT 50 OFFSET 0"
    );
    assert_eq!(
        page(Dialect::Sqlite, filters(), Combinator::And)?,
        "SELECT * FROM \"app\".\"users\" WHERE (\"id\" > ?1) AND \
         (lower(name) <> 'x' AND (org_id IS NULL OR org_id > 2)) AND \
         (\"region\" = ?2) LIMIT 50 OFFSET 0"
    );
    Ok(())
}

#[test]
fn raw_pages_and_counts_still_classify_as_plain_reads() -> Result<(), SelectError> {
    for dialect in [Dialect::Postgres, Dialect::MySql, Dialect::Sqlite] {
        let model = query(vec![raw("id IN (SELECT id FROM t)")], Combinator::And);
        for sql in [
            page_sql(dialect, &model, &known(), PAGE)?.sql,
            count_sql(dialect, &model, &known())?.sql,
        ] {
            let class = classify(&sql, dialect);
            assert_eq!(class.kind, StatementKind::Read, "{dialect:?}: {sql}");
            assert!(!class.dangerous, "{dialect:?}: {sql}");
        }
    }
    Ok(())
}

#[test]
fn raw_predicates_that_could_escape_their_parentheses_are_refused() {
    let cases: &[(Dialect, &str, &str)] = &[
        (Dialect::Postgres, "", "is empty"),
        (Dialect::Postgres, "   ", "is empty"),
        (
            Dialect::Postgres,
            "1=1) OR (1=1",
            "has unbalanced parentheses",
        ),
        (Dialect::Postgres, "(1=1", "has unbalanced parentheses"),
        (
            Dialect::Postgres,
            "1=1; DROP TABLE users",
            "must be one expression",
        ),
        (
            Dialect::Postgres,
            "1=1 -- hide the rest",
            "cannot contain comments",
        ),
        (Dialect::Postgres, "1=1 /* x */", "cannot contain comments"),
        (Dialect::MySql, "1=1 # x", "cannot contain comments"),
        (Dialect::MySql, "/*!50000 1=1 */", "cannot contain comments"),
        (
            Dialect::Postgres,
            "id = $1",
            "cannot contain bind parameters",
        ),
        (Dialect::MySql, "id = ?", "cannot contain bind parameters"),
        (Dialect::Sqlite, "id = ?1", "cannot contain bind parameters"),
        (Dialect::Postgres, "name = 'open", "unterminated"),
        (Dialect::MySql, "name = `open", "unterminated"),
    ];
    for &(dialect, text, why) in cases {
        let error = page(dialect, vec![raw(text)], Combinator::And)
            .err()
            .map(|error| error.to_string())
            .unwrap_or_default();
        assert!(error.contains(why), "{dialect:?} {text:?}: {error:?}");
    }
    assert_eq!(
        page(Dialect::Postgres, vec![raw("a\0")], Combinator::And),
        Err(SelectError::NulCharacter)
    );
}

#[test]
fn raw_predicates_ignore_the_column_and_keep_quoted_parentheses() -> Result<(), SelectError> {
    let mut quoted = raw("name = ')' OR \"region\" = '('");
    quoted.column = "not a column".to_owned();
    assert_eq!(
        page(Dialect::Postgres, vec![quoted], Combinator::And)?,
        "SELECT * FROM \"app\".\"users\" WHERE (name = ')' OR \"region\" = '(') LIMIT 50 OFFSET 0"
    );
    Ok(())
}

#[test]
fn foreign_key_jumps_are_bound_equality_filters_on_every_key_column() -> Result<(), SelectError> {
    let keys = || {
        vec![
            filter("org_id", FilterOp::Equal, "42"),
            filter("region", FilterOp::Equal, "eu'; --"),
        ]
    };
    let pg = page_sql(
        Dialect::Postgres,
        &query(keys(), Combinator::And),
        &known(),
        PAGE,
    )?;
    assert_eq!(
        pg.sql,
        "SELECT * FROM \"app\".\"users\" WHERE (\"org_id\" = $1) AND (\"region\" = $2) LIMIT 50 OFFSET 0"
    );
    assert_eq!(
        pg.params,
        vec![Some("42".to_owned()), Some("eu'; --".to_owned())]
    );
    assert_eq!(
        page(Dialect::MySql, keys(), Combinator::And)?,
        "SELECT * FROM `app`.`users` WHERE (`org_id` = ?) AND (`region` = ?) LIMIT 50 OFFSET 0"
    );
    assert_eq!(
        page(Dialect::Sqlite, keys(), Combinator::And)?,
        "SELECT * FROM \"app\".\"users\" WHERE (\"org_id\" = ?1) AND (\"region\" = ?2) LIMIT 50 OFFSET 0"
    );
    Ok(())
}
