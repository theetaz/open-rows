// SPDX-License-Identifier: Apache-2.0
//! Table-driven and property tests for the SQL toolkit.

mod classify;
mod classify_session;
mod filter_builder;
mod generate;
mod mysql;
mod mysql_split;
mod placeholders;
mod properties;
mod row_id;
mod select;
mod split;
mod sqlite;
mod stream;
mod tsql;
mod tsql_generate;
mod tsql_split;

use crate::{Classification, Dialect, StatementKind};

const PG: Dialect = Dialect::Postgres;

/// Statement texts produced by splitting `source`.
fn texts(source: &str) -> Vec<&str> {
    texts_in(source, PG)
}

/// Statement texts produced by splitting `source` in `dialect`.
fn texts_in(source: &str, dialect: Dialect) -> Vec<&str> {
    crate::split(source, dialect)
        .iter()
        .map(|statement| statement.text(source))
        .collect()
}

/// `(sql, kind, dangerous)` rows.
type Rows<'a> = &'a [(&'a str, StatementKind, bool)];

fn check(rows: Rows<'_>) {
    check_in(PG, rows);
}

fn check_in(dialect: Dialect, rows: Rows<'_>) {
    for &(sql, kind, dangerous) in rows {
        assert_eq!(
            crate::classify(sql, dialect),
            Classification { kind, dangerous },
            "{dialect:?} sql: {sql:?}"
        );
    }
}
mod clickhouse;

mod filter_groups;
