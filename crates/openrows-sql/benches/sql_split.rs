// SPDX-License-Identifier: Apache-2.0
//! `sql_split_*` (docs/13 §4): splitting 10 KB and 1 MB scripts per
//! dialect, reported as bytes/s. Timed: `split` only; the synthetic script
//! is built once per size and dialect.
use criterion::{Criterion, Throughput};
use openrows_sql::{Dialect, split};
use std::hint::black_box;

/// Statements typical of each dialect: comments, quoted `;`, and the
/// block forms the splitter must keep whole.
fn template(dialect: Dialect) -> &'static str {
    match dialect {
        Dialect::MySql => {
            "-- refresh totals; keep `;` in strings\n\
             UPDATE orders SET note = 'a;b', total = total * 1.1 WHERE id = 42;\n\
             SELECT `id`, \"x;y\" # trailing comment\n  FROM customers WHERE name LIKE 'O\\'Brien%';\n\
             DELIMITER //\n\
             CREATE PROCEDURE p_refresh() BEGIN UPDATE t SET a = 1; SELECT 2; END//\n\
             DELIMITER ;\n"
        }
        Dialect::Sqlite => {
            "-- refresh totals; keep `;` in strings\n\
             UPDATE orders SET note = 'a;b', total = total * 1.1 WHERE id = 42;\n\
             SELECT \"id\", [x;y] FROM customers /* block; comment */ WHERE name = 'O''Brien';\n\
             CREATE TRIGGER trg AFTER INSERT ON orders BEGIN\n  UPDATE totals SET n = n + 1; SELECT 1;\nEND;\n"
        }
        _ => {
            "-- refresh totals; keep `;` in strings\n\
             UPDATE orders SET note = 'a;b', total = total * 1.1 WHERE id = 42;\n\
             SELECT \"id\", E'x\\';y' FROM customers /* block; /* nested; */ comment */ WHERE name = 'O''Brien';\n\
             CREATE FUNCTION f() RETURNS int LANGUAGE plpgsql AS $body$ BEGIN PERFORM 1; RETURN 2; END $body$;\n"
        }
    }
}

/// `template` repeated to at least `bytes` bytes.
fn script(dialect: Dialect, bytes: usize) -> String {
    let unit = template(dialect);
    unit.repeat(bytes.div_ceil(unit.len()))
}

fn split_benchmarks(criterion: &mut Criterion) {
    let dialects = [
        ("postgres", Dialect::Postgres),
        ("mysql", Dialect::MySql),
        ("sqlite", Dialect::Sqlite),
    ];
    for (group, bytes) in [
        ("sql_split_10kb", 10 * 1024),
        ("sql_split_1mb", 1024 * 1024),
    ] {
        let mut group = criterion.benchmark_group(group);
        for (name, dialect) in dialects {
            let sql = script(dialect, bytes);
            if split(&sql, dialect).is_empty() {
                std::process::exit(1);
            }
            group.throughput(Throughput::Bytes(sql.len() as u64));
            group.bench_function(name, |bench| {
                bench.iter(|| black_box(split(black_box(&sql), dialect)));
            });
        }
        group.finish();
    }
}

fn main() {
    let mut criterion = Criterion::default().configure_from_args();
    split_benchmarks(&mut criterion);
    criterion.final_summary();
}
