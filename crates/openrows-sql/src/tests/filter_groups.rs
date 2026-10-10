// SPDX-License-Identifier: Apache-2.0
//! Group joins and shared parameter numbering across every SQL dialect.
use crate::Dialect;
use crate::select::{
    Combinator, Filter, FilterGroup, FilterOp, Page, TableQuery, TableRef, count_sql, page_sql,
};

fn row(column: &str, value: &str) -> Filter {
    Filter {
        column: column.into(),
        op: FilterOp::Equal,
        value: value.into(),
        second: String::new(),
    }
}

fn query() -> TableQuery {
    TableQuery {
        table: TableRef {
            schema: "app".into(),
            name: "items".into(),
        },
        columns: Vec::new(),
        filters: Vec::new(),
        combinator: Combinator::And,
        groups: vec![
            FilterGroup {
                filters: vec![row("id", "1"), row("id", "2")],
                combinator: Combinator::Or,
            },
            FilterGroup {
                filters: vec![
                    row("name", "x'; DROP TABLE items; --"),
                    Filter {
                        column: String::new(),
                        op: FilterOp::Raw,
                        value: "id > 0 OR id IS NULL".into(),
                        second: String::new(),
                    },
                ],
                combinator: Combinator::And,
            },
        ],
        sort: None,
        row_id: None,
    }
}

#[test]
fn groups_parenthesise_rows_and_bind_values_in_every_dialect()
-> Result<(), crate::select::SelectError> {
    let known = vec!["id".into(), "name".into()];
    // libSQL shares SQLite's generator and placeholder spelling.
    for dialect in [
        Dialect::Postgres,
        Dialect::MySql,
        Dialect::Sqlite,
        Dialect::SqlServer,
        Dialect::ClickHouse,
        Dialect::Sqlite,
    ] {
        let q = query();
        let bound = count_sql(dialect, &q, &known)?;
        let id = dialect.quote_ident("id")?;
        let name = dialect.quote_ident("name")?;
        let expected = format!(
            " WHERE (({id} = {}) OR ({id} = {})) AND (({name} = {}) AND (id > 0 OR id IS NULL))",
            dialect.placeholder(1),
            dialect.placeholder(2),
            dialect.placeholder(3)
        );
        assert!(bound.sql.ends_with(&expected), "{}", bound.sql);
        assert_eq!(
            bound.params,
            [
                Some("1".into()),
                Some("2".into()),
                Some("x'; DROP TABLE items; --".into())
            ]
        );
        assert!(!bound.sql.contains("DROP"));
        let page = page_sql(
            dialect,
            &q,
            &known,
            Page {
                limit: 30,
                offset: 0,
            },
        )?;
        assert!(page.sql.contains(&expected));
        assert_eq!(page.params, bound.params);
    }
    Ok(())
}

#[test]
fn outer_or_empty_groups_and_raw_checks_keep_their_semantics()
-> Result<(), crate::select::SelectError> {
    let known = vec!["id".into(), "name".into()];
    let mut q = query();
    q.combinator = Combinator::Or;
    q.groups.insert(
        0,
        FilterGroup {
            filters: Vec::new(),
            combinator: Combinator::And,
        },
    );
    assert!(
        count_sql(Dialect::Postgres, &q, &known)?
            .sql
            .contains(")) OR ((")
    );
    for unsafe_raw in [
        "id = $1",
        "id = 1; DELETE FROM items",
        "id = 1 -- x",
        "(id = 1",
        "id = 1) OR (1=1",
    ] {
        q.groups[2].filters[1].value = unsafe_raw.into();
        assert!(
            count_sql(Dialect::Postgres, &q, &known).is_err(),
            "{unsafe_raw}"
        );
    }
    Ok(())
}
