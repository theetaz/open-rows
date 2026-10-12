// SPDX-License-Identifier: Apache-2.0
//! Any-column search preserves filter joins and binds literal search values.
use crate::Dialect;
use crate::select::{
    Combinator, Filter, FilterGroup, FilterOp, Page, SelectError, TableQuery, TableRef, count_sql,
    page_sql,
};

fn query() -> TableQuery {
    let filter = |value: &str| Filter {
        column: "id".into(),
        op: FilterOp::Equal,
        value: value.into(),
        second: String::new(),
    };
    TableQuery {
        table: TableRef {
            schema: "app".into(),
            name: "items".into(),
        },
        columns: vec!["id".into()],
        filters: Vec::new(),
        groups: vec![
            FilterGroup {
                filters: vec![filter("1")],
                combinator: Combinator::And,
            },
            FilterGroup {
                filters: vec![filter("2")],
                combinator: Combinator::Or,
            },
        ],
        combinator: Combinator::Or,
        sort: None,
        search: Some("50%_'; DROP TABLE items; --".into()),
        row_id: None,
    }
}

#[test]
fn search_binds_every_catalog_column_and_ands_the_existing_or_groups() -> Result<(), SelectError> {
    let known = vec!["id".into(), "hidden name".into()];
    for dialect in [
        Dialect::Postgres,
        Dialect::MySql,
        Dialect::Sqlite,
        Dialect::SqlServer,
        Dialect::ClickHouse,
    ] {
        let count = count_sql(dialect, &query(), &known)?;
        let page = page_sql(
            dialect,
            &query(),
            &known,
            Page {
                limit: 10,
                offset: 0,
            },
        )?;
        assert_eq!(count.params, page.params);
        assert_eq!(count.params.len(), 4);
        assert!(count.sql.contains(")) OR (("), "{}", count.sql);
        assert!(count.sql.contains("))) AND ("), "{}", count.sql);
        assert!(count.sql.contains(&dialect.quote_ident("hidden name")?));
        assert!(!count.sql.contains("DROP"));
        assert!(
            count.params[2]
                .as_ref()
                .is_some_and(|text| text.contains("50\\%\\_"))
        );
    }
    Ok(())
}

#[test]
fn empty_search_preserves_sql_and_nul_search_is_rejected() -> Result<(), SelectError> {
    let known = vec!["id".into(), "name".into()];
    let mut query = query();
    query.search = None;
    let base = count_sql(Dialect::Postgres, &query, &known)?;
    query.search = Some("  ".into());
    assert_eq!(count_sql(Dialect::Postgres, &query, &known)?, base);
    query.search = Some("bad\0text".into());
    assert_eq!(
        count_sql(Dialect::Postgres, &query, &known),
        Err(SelectError::NulCharacter)
    );
    Ok(())
}

#[test]
fn search_requires_catalog_columns_before_generating_sql() {
    let mut query = query();
    query.columns.clear();
    query.groups.clear();
    assert_eq!(
        count_sql(Dialect::Postgres, &query, &[]),
        Err(SelectError::NoColumns)
    );
    assert_eq!(
        page_sql(
            Dialect::Postgres,
            &query,
            &[],
            Page {
                limit: 10,
                offset: 0
            }
        ),
        Err(SelectError::NoColumns)
    );
}
