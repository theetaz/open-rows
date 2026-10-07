// SPDX-License-Identifier: Apache-2.0
//! Table-driven and property tests for the SQL toolkit.

mod classify;
mod classify_session;
mod placeholders;
mod properties;
mod split;

use crate::{Classification, Dialect, StatementKind};

const PG: Dialect = Dialect::Postgres;

/// Statement texts produced by splitting `source`.
fn texts(source: &str) -> Vec<&str> {
    crate::split(source, PG)
        .iter()
        .map(|statement| statement.text(source))
        .collect()
}

/// `(sql, kind, dangerous)` rows.
type Rows<'a> = &'a [(&'a str, StatementKind, bool)];

fn check(rows: Rows<'_>) {
    for &(sql, kind, dangerous) in rows {
        assert_eq!(
            crate::classify(sql, PG),
            Classification { kind, dangerous },
            "sql: {sql:?}"
        );
    }
}
