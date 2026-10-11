// SPDX-License-Identifier: Apache-2.0
//! Admission and adversarial fixtures for every newly allowed query construct.
use super::*;
const DIALECTS: [Dialect; 3] = [Dialect::Postgres, Dialect::MySql, Dialect::Sqlite];
#[test]
fn joins_ctes_subqueries_aggregates_windows_and_union_are_admitted() {
    for dialect in DIALECTS {
        for sql in [
            "SELECT a.id FROM main.items AS a JOIN main.orders AS b ON a.id=b.id ORDER BY a.id LIMIT 20",
            "SELECT id FROM main.items WHERE EXISTS (SELECT 1 FROM main.orders WHERE orders.id=items.id)",
            "SELECT id FROM (SELECT id FROM main.items) AS selected",
            "WITH selected AS (SELECT id FROM main.items) SELECT id FROM selected",
            "WITH selected AS (SELECT id FROM main.items), totals AS (SELECT count(*) AS total FROM selected) SELECT * FROM totals",
            "SELECT id, count(*) AS total FROM main.items GROUP BY id HAVING count(*) > 3",
            "SELECT id, row_number() OVER (PARTITION BY id ORDER BY id) AS position FROM main.items",
            "SELECT id FROM main.items UNION ALL SELECT id FROM main.orders ORDER BY id LIMIT 10",
        ] {
            assert!(validate_ai_read(sql, dialect).is_ok(), "{dialect:?}: {sql}");
        }
    }
}
#[test]
fn every_write_side_effect_and_nested_attack_is_refused() {
    for dialect in DIALECTS {
        for sql in [
            "DELETE FROM t",
            "WITH x AS (DELETE FROM t RETURNING *) SELECT * FROM x",
            "WITH x AS (SELECT 1), y AS (UPDATE t SET id=2 RETURNING *) SELECT * FROM x",
            "SELECT * INTO t FROM s",
            "SELECT pg_terminate_backend(1)",
            "SELECT pg_catalog.pg_terminate_backend(1)",
            "SELECT \"pg_terminate_backend\"(1)",
            "SELECT nextval('s')",
            "SELECT setval('s',1)",
            "SELECT pg_sleep(0)",
            "SELECT dblink('x','SELECT 1')",
            "SELECT lo_import('x')",
            "SELECT pg_advisory_lock(1)",
            "SELECT set_config('x','y',false)",
            "SELECT SLEEP(0)",
            "SELECT GET_LOCK('x',0)",
            "SELECT LOAD_FILE('x')",
            "SELECT load_extension('x')",
            "ATTACH DATABASE 'x' AS other",
            "SELECT id FROM main.items INTO OUTFILE 'x'",
            "SELECT 1; DROP TABLE t",
            "COPY t TO PROGRAM 'x'",
            "SELECT * FROM main.items FOR UPDATE",
            "SELECT @x:=1",
            "SELECT 1::user_type",
            "SELECT CAST(1 AS user_type)",
            "SELECT /*!50000 get_lock('x',1) */ 1",
            "SELECT /*m!100100 get_lock('x',1) */ 1",
            "SELECT /*+ SET_VAR(max_execution_time=0) */ 1",
            "SELECT unknown_func()",
            "SELECT id FROM main.items WHERE id !! 1",
            "SELECT id FROM main.items WHERE id ** 1",
            "SELECT 1 UNION SELECT nextval('s')",
            "SELECT count((SELECT unknown_func())) FROM main.items",
            "SELECT row_number() OVER (ORDER BY pg_sleep(1)) FROM main.items",
            "WITH x AS (SELECT unknown_func()) SELECT * FROM x",
            "SELECT id FROM main.items JOIN main.orders ON unknown_func()=1",
            "SELECT * FROM unknown_func()",
            "SELECT public.count(*) FROM main.items",
            "SELECT email hidden FROM main.items",
            "SELECT email AS hidden FROM main.items",
        ] {
            assert!(
                validate_ai_read(sql, dialect).is_err(),
                "{dialect:?}: {sql}"
            );
        }
    }
}
#[test]
fn all_sources_are_checked_and_ambiguous_samples_lose_provenance() {
    let sql =
        "WITH x AS (SELECT id FROM main.items) SELECT id FROM x UNION SELECT id FROM main.orders";
    assert_eq!(
        ai_read_sources(sql, Dialect::Sqlite).unwrap(),
        vec![
            ("main".into(), "items".into()),
            ("main".into(), "orders".into())
        ]
    );
    assert_eq!(ai_read_source(sql, Dialect::Sqlite).unwrap(), None);
    assert_eq!(
        ai_read_source("SELECT email FROM main.items", Dialect::Sqlite).unwrap(),
        Some(("main".into(), "items".into()))
    );
    assert!(
        validate_ai_read(
            &format!("SELECT {}1{}", "(".repeat(33), ")".repeat(33)),
            Dialect::Sqlite
        )
        .is_err()
    );
}

#[test]
fn flat_expression_chains_and_oversized_input_fail_before_ast_recursion() {
    for dialect in DIALECTS {
        let chain = format!("SELECT {}1", "1+".repeat(700));
        assert!(validate_ai_read(&chain, dialect).is_err());
        assert!(validate_ai_read(&format!("{}SELECT 1", " ".repeat(16384)), dialect).is_err());
    }
}

#[test]
fn self_and_forward_cte_names_cannot_bypass_base_relation_checks() {
    for dialect in DIALECTS {
        for sql in [
            "WITH ai_view AS (SELECT * FROM ai_view) SELECT * FROM ai_view",
            "WITH x AS (SELECT * FROM ai_view), ai_view AS (SELECT * FROM main.items) SELECT * FROM x",
        ] {
            assert!(
                validate_ai_read(sql, dialect).is_err(),
                "{dialect:?}: {sql}"
            );
        }
    }
}

#[test]
fn window_names_without_over_cannot_resolve_as_older_server_user_functions() {
    for dialect in DIALECTS {
        for call in [
            "row_number()",
            "rank()",
            "dense_rank()",
            "lag(1)",
            "lead(1)",
            "first_value(1)",
            "last_value(1)",
            "nth_value(1,1)",
            "ntile(1)",
            "percent_rank()",
            "cume_dist()",
        ] {
            assert!(
                validate_ai_read(&format!("SELECT {call}"), dialect).is_err(),
                "{dialect:?}: {call}"
            );
        }
    }
}

#[test]
fn mysql_function_gaps_and_mode_dependent_escapes_are_refused() {
    for sql in [
        "SELECT count (1)",
        "SELECT sum\n(1)",
        "SELECT max/**/(1)",
        "SELECT min -- comment\n(1)",
        "SELECT 'a\\b'",
    ] {
        assert!(validate_ai_read(sql, Dialect::MySql).is_err(), "{sql}");
    }
    for dialect in [Dialect::Postgres, Dialect::Sqlite] {
        assert!(validate_ai_read("SELECT count (1)", dialect).is_ok());
        assert!(validate_ai_read("SELECT 'a\\b'", dialect).is_err());
    }
    assert!(validate_ai_read("SELECT count(1), 'a''b'", Dialect::MySql).is_ok());
}

#[test]
fn postgres_relation_and_cte_names_follow_native_case_resolution() {
    let dialect = Dialect::Postgres;
    assert_eq!(
        ai_read_sources("SELECT id FROM MAIN.ITEMS", dialect).unwrap(),
        vec![("main".into(), "items".into())]
    );
    assert_eq!(
        ai_read_source("SELECT id FROM MAIN.ITEMS", dialect).unwrap(),
        Some(("main".into(), "items".into()))
    );
    assert_eq!(
        ai_read_sources("SELECT id FROM \"Main\".\"Items\"", dialect).unwrap(),
        vec![("Main".into(), "Items".into())]
    );
    assert!(
        validate_ai_read(
            "WITH \"ITEMS\" AS (SELECT id FROM main.items) SELECT * FROM ITEMS",
            dialect
        )
        .is_err()
    );
    assert!(
        validate_ai_read(
            "WITH ITEMS AS (SELECT id FROM main.items) SELECT * FROM items",
            dialect
        )
        .is_ok()
    );
    assert!(
        validate_ai_read(
            "WITH \"ITEMS\" AS (SELECT id FROM main.items) SELECT * FROM \"ITEMS\"",
            dialect
        )
        .is_ok()
    );
    assert!(validate_ai_read("SELECT * FROM main.Ö", dialect).is_err());
}
