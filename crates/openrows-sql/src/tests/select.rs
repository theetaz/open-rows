// SPDX-License-Identifier: Apache-2.0
//! Table-browsing SQL: quoting, every operator, NULL handling, pagination and
//! injection attempts as values and as identifiers.
use crate::Dialect;
use crate::select::{
    BoundSql, Combinator, Filter, FilterOp, Page, SelectError, Sort, SortDirection, TableQuery,
    TableRef, count_sql, page_sql,
};

fn known() -> Vec<String> {
    ["id", "name", "Mixed Case", "say \"hi\""]
        .map(str::to_owned)
        .to_vec()
}

fn query() -> TableQuery {
    TableQuery {
        table: TableRef {
            schema: "public".to_owned(),
            name: "users".to_owned(),
        },
        columns: Vec::new(),
        filters: Vec::new(),
        combinator: Combinator::And,
        sort: None,
    }
}

fn filter(column: &str, op: FilterOp, value: &str) -> Filter {
    Filter {
        column: column.to_owned(),
        op,
        value: value.to_owned(),
        second: String::new(),
    }
}

fn with(filters: Vec<Filter>) -> TableQuery {
    TableQuery { filters, ..query() }
}

fn page(query: &TableQuery) -> Result<BoundSql, SelectError> {
    page_sql(
        Dialect::Postgres,
        query,
        &known(),
        Page {
            limit: 300,
            offset: 0,
        },
    )
}

fn some(values: &[&str]) -> Vec<Option<String>> {
    values
        .iter()
        .map(|value| Some((*value).to_owned()))
        .collect()
}

#[test]
fn identifiers_are_double_quoted_with_quotes_doubled() {
    assert_eq!(
        Dialect::Postgres.quote_ident("users"),
        Ok("\"users\"".to_owned())
    );
    assert_eq!(
        Dialect::Postgres.quote_ident("Mixed Case"),
        Ok("\"Mixed Case\"".to_owned())
    );
    assert_eq!(
        Dialect::Postgres.quote_ident("a\"b"),
        Ok("\"a\"\"b\"".to_owned())
    );
    assert_eq!(Dialect::Postgres.quote_ident(""), Ok("\"\"".to_owned()));
    assert_eq!(
        Dialect::Postgres.quote_ident("x\0"),
        Err(SelectError::NulCharacter)
    );
}

#[test]
fn plain_page_selects_everything_with_limit_and_offset() {
    let bound = page_sql(
        Dialect::Postgres,
        &query(),
        &known(),
        Page {
            limit: 300,
            offset: 600,
        },
    );
    assert_eq!(
        bound,
        Ok(BoundSql {
            sql: "SELECT * FROM \"public\".\"users\" LIMIT 300 OFFSET 600".to_owned(),
            params: Vec::new(),
        })
    );
}

#[test]
fn table_and_schema_names_are_quoted() -> Result<(), SelectError> {
    let mut odd = query();
    odd.table = TableRef {
        schema: "My \"Schema\"".to_owned(),
        name: "t; DROP TABLE x".to_owned(),
    };
    let bound = page(&odd)?;
    assert!(
        bound
            .sql
            .starts_with("SELECT * FROM \"My \"\"Schema\"\"\".\"t; DROP TABLE x\" LIMIT")
    );
    Ok(())
}

#[test]
fn projection_lists_known_columns_in_order() -> Result<(), SelectError> {
    let mut projected = query();
    projected.columns = vec!["name".to_owned(), "say \"hi\"".to_owned()];
    let bound = page(&projected)?;
    assert!(
        bound
            .sql
            .starts_with("SELECT \"name\", \"say \"\"hi\"\"\" FROM")
    );
    projected.columns = vec!["name, (SELECT 1)".to_owned()];
    assert_eq!(
        page(&projected),
        Err(SelectError::UnknownColumn("name, (SELECT 1)".to_owned()))
    );
    Ok(())
}

#[test]
fn comparisons_bind_their_values() -> Result<(), SelectError> {
    let cases = [
        (FilterOp::Equal, "="),
        (FilterOp::NotEqual, "<>"),
        (FilterOp::Less, "<"),
        (FilterOp::Greater, ">"),
        (FilterOp::LessOrEqual, "<="),
        (FilterOp::GreaterOrEqual, ">="),
    ];
    for (op, symbol) in cases {
        let bound = page(&with(vec![filter("id", op, "42")]))?;
        assert_eq!(
            bound.sql,
            format!(
                "SELECT * FROM \"public\".\"users\" WHERE (\"id\" {symbol} $1) LIMIT 300 OFFSET 0"
            )
        );
        assert_eq!(bound.params, some(&["42"]));
    }
    Ok(())
}

#[test]
fn patterns_use_ilike_on_text_and_escape_wildcards() -> Result<(), SelectError> {
    let cases = [
        (FilterOp::Contains, "ILIKE", "%50\\%\\_off\\\\%"),
        (FilterOp::NotContains, "NOT ILIKE", "%50\\%\\_off\\\\%"),
        (FilterOp::StartsWith, "ILIKE", "50\\%\\_off\\\\%"),
        (FilterOp::EndsWith, "ILIKE", "%50\\%\\_off\\\\"),
    ];
    for (op, operator, value) in cases {
        let bound = page(&with(vec![filter("name", op, "50%_off\\")]))?;
        assert!(
            bound
                .sql
                .contains(&format!("WHERE (\"name\"::text {operator} $1)")),
            "{}",
            bound.sql
        );
        assert_eq!(bound.params, some(&[value]));
    }
    Ok(())
}

#[test]
fn in_lists_bind_each_trimmed_item() -> Result<(), SelectError> {
    let bound = page(&with(vec![filter("id", FilterOp::In, " 1, 2 ,,3 ")]))?;
    assert!(bound.sql.contains("WHERE (\"id\" IN ($1, $2, $3))"));
    assert_eq!(bound.params, some(&["1", "2", "3"]));
    assert_eq!(
        page(&with(vec![filter("id", FilterOp::In, " , ")])),
        Err(SelectError::EmptyList("id".to_owned()))
    );
    Ok(())
}

#[test]
fn null_checks_take_no_parameters() -> Result<(), SelectError> {
    let bound = page(&with(vec![
        filter("name", FilterOp::IsNull, "ignored"),
        filter("id", FilterOp::IsNotNull, ""),
    ]))?;
    assert!(
        bound
            .sql
            .contains("WHERE (\"name\" IS NULL) AND (\"id\" IS NOT NULL)")
    );
    assert!(bound.params.is_empty());
    Ok(())
}

#[test]
fn between_binds_both_bounds() -> Result<(), SelectError> {
    let mut range = filter("id", FilterOp::Between, "10");
    range.second = "20".to_owned();
    let bound = page(&with(vec![range]))?;
    assert!(bound.sql.contains("WHERE (\"id\" BETWEEN $1 AND $2)"));
    assert_eq!(bound.params, some(&["10", "20"]));
    Ok(())
}

#[test]
fn placeholders_number_across_filters_and_combinators() -> Result<(), SelectError> {
    let mut query = with(vec![
        filter("id", FilterOp::Greater, "1"),
        filter("name", FilterOp::In, "a,b"),
        filter("Mixed Case", FilterOp::Equal, "x"),
    ]);
    query.combinator = Combinator::Or;
    let bound = page(&query)?;
    assert!(
        bound.sql.contains(
            "WHERE (\"id\" > $1) OR (\"name\" IN ($2, $3)) OR (\"Mixed Case\" = $4) LIMIT"
        )
    );
    assert_eq!(bound.params, some(&["1", "a", "b", "x"]));
    Ok(())
}

#[test]
fn injection_attempts_in_values_stay_parameters() -> Result<(), SelectError> {
    let attack = "'; DROP TABLE users; --";
    let bound = page(&with(vec![
        filter("name", FilterOp::Equal, attack),
        filter("name", FilterOp::Contains, attack),
    ]))?;
    assert!(!bound.sql.contains("DROP"));
    assert!(!bound.sql.contains('\''));
    assert_eq!(bound.params.first(), Some(&Some(attack.to_owned())));
    assert_eq!(
        page(&with(vec![filter("name", FilterOp::Equal, "a\0b")])),
        Err(SelectError::NulCharacter)
    );
    Ok(())
}

#[test]
fn unknown_columns_are_refused_everywhere() {
    let attack = "id\" = \"id\"; DROP TABLE users; --";
    assert_eq!(
        page(&with(vec![filter(attack, FilterOp::Equal, "1")])),
        Err(SelectError::UnknownColumn(attack.to_owned()))
    );
    let mut sorted = query();
    sorted.sort = Some(Sort {
        column: "1; DROP TABLE users".to_owned(),
        direction: SortDirection::Ascending,
    });
    assert!(matches!(page(&sorted), Err(SelectError::UnknownColumn(_))));
    assert_eq!(
        page_sql(
            Dialect::Postgres,
            &query(),
            &[],
            Page {
                limit: 300,
                offset: 0
            }
        ),
        Err(SelectError::NoColumns)
    );
}

#[test]
fn sort_orders_by_a_quoted_column() -> Result<(), SelectError> {
    let mut sorted = query();
    sorted.sort = Some(Sort {
        column: "Mixed Case".to_owned(),
        direction: SortDirection::Descending,
    });
    let bound = page_sql(
        Dialect::Postgres,
        &sorted,
        &known(),
        Page {
            limit: 50,
            offset: 100,
        },
    )?;
    assert!(
        bound
            .sql
            .ends_with("ORDER BY \"Mixed Case\" DESC LIMIT 50 OFFSET 100")
    );
    sorted.sort = Some(Sort {
        column: "id".to_owned(),
        direction: SortDirection::Ascending,
    });
    assert!(page(&sorted)?.sql.contains("ORDER BY \"id\" ASC LIMIT"));
    Ok(())
}

#[test]
fn counts_share_filters_but_ignore_sort_and_projection() -> Result<(), SelectError> {
    let mut query = with(vec![filter("id", FilterOp::Less, "5")]);
    query.columns = vec!["name".to_owned()];
    query.sort = Some(Sort {
        column: "id".to_owned(),
        direction: SortDirection::Ascending,
    });
    let bound = count_sql(Dialect::Postgres, &query, &known())?;
    assert_eq!(
        bound.sql,
        "SELECT count(*) FROM \"public\".\"users\" WHERE (\"id\" < $1)"
    );
    assert_eq!(bound.params, some(&["5"]));
    Ok(())
}

#[test]
fn generated_pages_classify_as_reads() -> Result<(), SelectError> {
    let bound = page(&with(vec![filter("name", FilterOp::Contains, "x")]))?;
    let class = crate::classify(&bound.sql, Dialect::Postgres);
    assert_eq!(class.kind, crate::StatementKind::Read);
    assert!(!class.dangerous);
    Ok(())
}

#[test]
fn errors_explain_themselves() {
    assert!(
        SelectError::UnknownColumn("x".to_owned())
            .to_string()
            .contains("\"x\"")
    );
    assert!(
        SelectError::EmptyList("id".to_owned())
            .to_string()
            .contains("IN")
    );
    assert!(SelectError::NoColumns.to_string().contains("no columns"));
    assert!(SelectError::NulCharacter.to_string().contains("NUL"));
}

#[test]
fn expanded_operators_keep_patterns_and_lists_in_parameters() -> Result<(), SelectError> {
    for (op, operator) in [
        (FilterOp::NotIn, "NOT IN"),
        (FilterOp::Like, "ILIKE"),
        (FilterOp::NotLike, "NOT ILIKE"),
        (FilterOp::Regex, "~"),
        (FilterOp::NotRegex, "!~"),
    ] {
        let bound = page(&with(vec![filter("name", op, "'; DROP TABLE users; --")]))?;
        assert!(bound.sql.contains(operator));
        assert!(!bound.sql.contains("DROP"));
        assert_eq!(bound.params, some(&["'; DROP TABLE users; --"]));
    }
    let mut range = filter("id", FilterOp::NotBetween, "2");
    range.second = "9".into();
    let bound = page(&with(vec![range]))?;
    assert!(bound.sql.contains("NOT BETWEEN $1 AND $2"));
    assert_eq!(bound.params, some(&["2", "9"]));
    for (op, operator) in [(FilterOp::IsEmpty, "= ''"), (FilterOp::IsNotEmpty, "<> ''")] {
        let bound = page(&with(vec![filter("name", op, "ignored")]))?;
        assert!(bound.sql.contains(operator));
        assert!(bound.params.is_empty());
    }
    Ok(())
}
