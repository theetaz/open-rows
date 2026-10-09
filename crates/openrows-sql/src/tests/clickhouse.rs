// SPDX-License-Identifier: Apache-2.0
use crate::{Dialect, ParamBinder, ParamStyle, classify, limit_select, split};
const CH: Dialect = Dialect::ClickHouse;
#[test]
fn analytical_mutations_are_writes() {
    for sql in [
        "INSERT INTO t VALUES (1)",
        "ALTER TABLE t UPDATE x=1 WHERE id=2",
        "ALTER TABLE t DELETE WHERE id=1",
        "OPTIMIZE TABLE t FINAL",
        "SYSTEM RELOAD DICTIONARY d",
        "KILL QUERY WHERE query_id='x'",
        "DROP TABLE t",
        "TRUNCATE TABLE t",
    ] {
        assert!(classify(sql, CH).is_write(), "{sql}");
    }
}
#[test]
fn quotes_split_and_parameters_follow_clickhouse() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(split("SELECT 'a\\\';b', `x\\`y`; SELECT 2", CH).len(), 2);
    assert_eq!(CH.quote_ident("a`b\\c")?, "`a\\`b\\\\c`");
    let sql = "SELECT :value";
    let mut binder = ParamBinder::new(CH, ParamStyle::Named);
    let slots = binder.slots(sql);
    let bound = binder.bind(sql, &slots, |_| Some(Some("'\\".to_owned())))?;
    assert_eq!(bound.sql, "SELECT {p1:Nullable(String)}");
    assert_eq!(bound.params, [Some("'\\".to_owned())]);
    assert!(limit_select("SELECT 1", CH, 100).is_some());
    Ok(())
}

#[test]
fn typed_native_parameters_keep_their_declared_types() -> Result<(), Box<dyn std::error::Error>> {
    let sql = "SELECT {n:UInt64}, {value:Nullable(String)}, '{fake:Int32}'";
    let mut binder = ParamBinder::new(CH, ParamStyle::Named);
    let slots = binder.slots(sql);
    assert_eq!(slots.len(), 2);
    let bound = binder.bind(sql, &slots, |_| Some(Some("1".to_owned())))?;
    assert_eq!(
        bound.sql,
        "SELECT {p1:UInt64}, {p2:Nullable(String)}, '{fake:Int32}'"
    );
    assert_eq!(
        limit_select("SELECT 1 SETTINGS max_threads=1", CH, 50).as_deref(),
        Some("SELECT 1 LIMIT 50 SETTINGS max_threads=1")
    );
    assert_eq!(
        limit_select("SELECT 1 FORMAT CSV", CH, 50).as_deref(),
        Some("SELECT 1 LIMIT 50 FORMAT CSV")
    );
    Ok(())
}

#[test]
fn database_context_is_replayed_after_reconnect() {
    assert_eq!(
        crate::session_context("USE `warehouse`", CH),
        Some(crate::ContextSetting::Database)
    );
}
