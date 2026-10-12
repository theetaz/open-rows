// SPDX-License-Identifier: Apache-2.0
//! Opt-in flat-reference versus grouped predicate generation; no wall-time assertions.
use criterion::Criterion;
use openrows_sql::{
    Dialect,
    select::{Combinator, Filter, FilterGroup, FilterOp, TableQuery, TableRef, count_sql},
};
use std::hint::black_box;

fn query(grouped: bool) -> TableQuery {
    let filters: Vec<Filter> = (0..8)
        .map(|n| Filter {
            column: "id".into(),
            op: FilterOp::Equal,
            value: n.to_string(),
            second: String::new(),
        })
        .collect();
    let groups = if grouped {
        filters
            .chunks(4)
            .map(|rows| FilterGroup {
                filters: rows.to_vec(),
                combinator: Combinator::Or,
            })
            .collect()
    } else {
        Vec::new()
    };
    TableQuery {
        table: TableRef {
            schema: "public".into(),
            name: "items".into(),
        },
        columns: Vec::new(),
        filters: if grouped { Vec::new() } else { filters },
        groups,
        combinator: Combinator::And,
        sort: None,
        search: None,
        row_id: None,
    }
}

fn main() {
    if std::env::var_os("OPENROWS_BENCH").is_none() {
        return;
    }
    let mut criterion = Criterion::default().configure_from_args();
    let known = vec!["id".to_owned()];
    for (name, grouped) in [("filter_flat_8", false), ("filter_groups_2x4", true)] {
        let query = query(grouped);
        criterion.bench_function(name, |bench| {
            bench.iter(|| black_box(count_sql(Dialect::Postgres, black_box(&query), &known)));
        });
    }
    criterion.final_summary();
}
