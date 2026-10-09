// SPDX-License-Identifier: Apache-2.0
//! Statement splitter, statement-at-cursor and classification (M1-15) for
//! each dialect: ranges stay in bounds, on character boundaries and in
//! order; every cursor maps to a statement when there is one; nothing panics.
#![no_main]

use libfuzzer_sys::fuzz_target;
use openrows_sql::{Dialect, classify, split, statement_at};

const DIALECTS: [Dialect; 3] = [Dialect::Postgres, Dialect::MySql, Dialect::Sqlite];

fuzz_target!(|data: &[u8]| {
    let Some((&selector, rest)) = data.split_first() else {
        return;
    };
    let dialect = DIALECTS[usize::from(selector) % DIALECTS.len()];
    let sql = String::from_utf8_lossy(rest);
    let statements = split(&sql, dialect);
    let mut end = 0;
    for statement in &statements {
        let range = &statement.range;
        assert!(end <= range.start && range.start < range.end && range.end <= sql.len());
        assert!(sql.get(range.clone()).is_some(), "range splits a character");
        end = range.end;
        let _ = classify(statement.text(&sql), dialect);
        if dialect == Dialect::Postgres {
            assert_eq!(split(statement.text(&sql), dialect).len(), 1);
        }
    }
    for cursor in [
        0,
        usize::from(selector),
        sql.len() / 2,
        sql.len(),
        usize::MAX,
    ] {
        let found = statement_at(&statements, cursor);
        assert_eq!(found.is_none(), statements.is_empty());
        assert!(found.is_none_or(|index| index < statements.len()));
    }
});
