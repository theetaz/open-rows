// SPDX-License-Identifier: Apache-2.0
use super::{TestResult, export, export_one, mixed, texts};
use crate::cell::Cell;
use crate::sql::push_literal;
use crate::text::Temporal;
use crate::{ExportError, Exporter, Format, InsertOptions};
use arrow_array::{ArrayRef, Int64Array, RecordBatch};
use arrow_schema::{DataType, Field, Schema, TimeUnit};
use openrows_sql::Dialect;
use std::borrow::Cow;
use std::num::NonZeroUsize;
use std::sync::Arc;

fn insert(dialect: Dialect, table: &str, rows: usize) -> TestResult<Format> {
    Ok(Format::Insert(InsertOptions {
        dialect,
        table: table.to_owned(),
        rows_per_statement: NonZeroUsize::new(rows).ok_or("zero rows")?,
    }))
}

fn literal(dialect: Dialect, cell: &Cell<'_>) -> String {
    let (mut scratch, mut out) = (String::new(), String::new());
    push_literal(dialect, cell, &mut scratch, &mut out);
    out
}

const ALL: [Dialect; 3] = [Dialect::Postgres, Dialect::MySql, Dialect::Sqlite];

#[test]
fn postgres_rows_use_typed_literals() -> TestResult {
    let text = export_one(&insert(Dialect::Postgres, "my \"table\"", 100)?, &mixed()?)?;
    assert_eq!(
        text,
        "INSERT INTO \"my \"\"table\"\"\" (\"id\", \"big\", \"ok\", \"ratio\", \"price\", \
         \"name\", \"doc\", \"raw\", \"uid\", \"day\", \"at\") VALUES\n\
         (-7, 18446744073709551615, TRUE, 0.5, -12.34, 'it''s \"ü\", ok', '{\"a\": [1, 2]}', \
         decode('00ff10', 'hex'), '550e8400-e29b-41d4-a716-446655440000', '2024-02-29', \
         '2024-02-29 12:34:56.123456+00:00'),\n\
         (NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL);\n"
    );
    Ok(())
}

#[test]
fn mysql_and_sqlite_quote_names_and_bytes_their_own_way() -> TestResult {
    let batch = mixed()?.project(&[0, 2, 7])?;
    let mysql = export_one(&insert(Dialect::MySql, "or`ders", 100)?, &batch)?;
    assert_eq!(
        mysql,
        "INSERT INTO `or``ders` (`id`, `ok`, `raw`) VALUES\n(-7, TRUE, X'00ff10'),\n(NULL, NULL, NULL);\n"
    );
    let sqlite = export_one(&insert(Dialect::Sqlite, "t", 100)?, &batch)?;
    assert_eq!(
        sqlite,
        "INSERT INTO \"t\" (\"id\", \"ok\", \"raw\") VALUES\n(-7, 1, X'00ff10'),\n(NULL, NULL, NULL);\n"
    );
    Ok(())
}

#[test]
fn rows_are_grouped_into_statements_across_batches() -> TestResult {
    let schema = Arc::new(Schema::new(vec![Field::new("n", DataType::Int64, false)]));
    let batch = |values: Vec<i64>| -> TestResult<RecordBatch> {
        let column: ArrayRef = Arc::new(Int64Array::from(values));
        Ok(RecordBatch::try_new(schema.clone(), vec![column])?)
    };
    let batches = [batch(vec![1, 2, 3])?, batch(vec![4, 5])?];
    let text = export(
        &insert(Dialect::Postgres, "t", 2)?,
        schema.clone(),
        &batches,
    )?;
    assert_eq!(
        text,
        "INSERT INTO \"t\" (\"n\") VALUES\n(1),\n(2);\n\
         INSERT INTO \"t\" (\"n\") VALUES\n(3),\n(4);\n\
         INSERT INTO \"t\" (\"n\") VALUES\n(5);\n"
    );
    let single = export(
        &insert(Dialect::Sqlite, "t", 1)?,
        schema.clone(),
        &batches[1..],
    )?;
    assert_eq!(
        single,
        "INSERT INTO \"t\" (\"n\") VALUES\n(4);\nINSERT INTO \"t\" (\"n\") VALUES\n(5);\n"
    );
    assert_eq!(export(&insert(Dialect::MySql, "t", 3)?, schema, &[])?, "");
    Ok(())
}

#[test]
fn strings_escape_per_dialect() {
    let cases = [
        ("plain", ["'plain'", "'plain'", "'plain'"]),
        ("it's", ["'it''s'", "'it\\'s'", "'it''s'"]),
        ("a\\b", ["E'a\\\\b'", "'a\\\\b'", "'a\\b'"]),
        ("o'\\n", ["E'o''\\\\n'", "'o\\'\\\\n'", "'o''\\n'"]),
        (
            "line\nnext\r",
            ["'line\nnext\r'", "'line\\nnext\\r'", "'line\nnext\r'"],
        ),
        (
            "nul\0ctrl\u{1a}",
            ["'nul\0ctrl\u{1a}'", "'nul\\0ctrl\\Z'", "'nul\0ctrl\u{1a}'"],
        ),
        (
            "ünï 😀 \"q\"",
            ["'ünï 😀 \"q\"'", "'ünï 😀 \"q\"'", "'ünï 😀 \"q\"'"],
        ),
        ("", ["''", "''", "''"]),
    ];
    for (text, expected) in cases {
        for (dialect, want) in ALL.into_iter().zip(expected) {
            assert_eq!(
                literal(dialect, &Cell::Text(text)),
                want,
                "{dialect:?} {text:?}"
            );
        }
    }
}

#[test]
fn numbers_booleans_and_nulls_are_bare() {
    for dialect in ALL {
        assert_eq!(literal(dialect, &Cell::Null), "NULL");
        assert_eq!(
            literal(dialect, &Cell::Int(i64::MIN)),
            "-9223372036854775808"
        );
        assert_eq!(
            literal(dialect, &Cell::UInt(u64::MAX)),
            "18446744073709551615"
        );
        assert_eq!(literal(dialect, &Cell::F64(-2.5)), "-2.5");
        assert_eq!(literal(dialect, &Cell::F32(0.1)), "0.1");
        assert_eq!(literal(dialect, &Cell::F64(1e-7)), "1e-7");
        assert_eq!(literal(dialect, &Cell::F64(-2.5e-300)), "-2.5e-300");
        assert_eq!(literal(dialect, &Cell::F64(1e300)), "1e300");
        assert_eq!(literal(dialect, &Cell::F64(123_456.75)), "123456.75");
        assert_eq!(literal(dialect, &Cell::F32(1e20)), "1e20");
        let exact = Cell::Number(Cow::Borrowed("123456789012345678901234567890.000001"));
        assert_eq!(
            literal(dialect, &exact),
            "123456789012345678901234567890.000001"
        );
    }
    let booleans = |dialect| {
        (
            literal(dialect, &Cell::Bool(true)),
            literal(dialect, &Cell::Bool(false)),
        )
    };
    assert_eq!(booleans(Dialect::Postgres), ("TRUE".into(), "FALSE".into()));
    assert_eq!(booleans(Dialect::MySql), ("TRUE".into(), "FALSE".into()));
    assert_eq!(booleans(Dialect::Sqlite), ("1".into(), "0".into()));
    assert_eq!(booleans(Dialect::SqlServer), ("1".into(), "0".into()));
}

#[test]
fn sql_server_literals_are_unicode_and_hex() {
    let server = Dialect::SqlServer;
    assert_eq!(literal(server, &Cell::Binary(&[0, 255])), "0x00ff");
    let mut out = String::new();
    crate::sql::push_string(server, "it's \u{4e16}", &mut out);
    assert_eq!(out, "N'it''s \u{4e16}'");
}

#[test]
fn non_finite_floats_follow_what_each_dialect_can_store() {
    let render = |value: f64| ALL.map(|dialect| literal(dialect, &Cell::F64(value)));
    assert_eq!(render(f64::NAN), ["'NaN'", "NULL", "NULL"]);
    assert_eq!(render(f64::INFINITY), ["'Infinity'", "NULL", "9e999"]);
    assert_eq!(render(f64::NEG_INFINITY), ["'-Infinity'", "NULL", "-9e999"]);
    let narrow = ALL.map(|dialect| literal(dialect, &Cell::F32(f32::NAN)));
    assert_eq!(narrow, ["'NaN'", "NULL", "NULL"]);
}

#[test]
fn binary_dates_and_uuids_are_quoted_or_hex() {
    let bytes = Cell::Binary(&[0xde, 0xad, 0x00, 0x27]);
    assert_eq!(
        ALL.map(|dialect| literal(dialect, &bytes)),
        ["decode('dead0027', 'hex')", "X'dead0027'", "X'dead0027'"]
    );
    let empty = Cell::Binary(&[]);
    assert_eq!(
        ALL.map(|dialect| literal(dialect, &empty)),
        ["decode('', 'hex')", "X''", "X''"]
    );
    let date = Cell::Temporal(Temporal::Date(-719_162));
    assert_eq!(literal(Dialect::MySql, &date), "'0001-01-01'");
    let local = Cell::Temporal(Temporal::Timestamp {
        value: 0,
        unit: TimeUnit::Microsecond,
        utc: false,
    });
    assert_eq!(literal(Dialect::Sqlite, &local), "'1970-01-01 00:00:00'");
    let uuid = Cell::Uuid(&[0xab; 16]);
    assert_eq!(
        literal(Dialect::Postgres, &uuid),
        "'abababab-abab-abab-abab-abababababab'"
    );
}

#[test]
fn decimal_text_that_is_not_a_number_stays_quoted() -> TestResult {
    let field = super::tagged(
        "n",
        DataType::Utf8,
        openrows_driver_api::metadata::DECIMAL_EXTENSION,
    );
    let schema = Arc::new(Schema::new(vec![field]));
    let column: ArrayRef = Arc::new(arrow_array::StringArray::from(vec![
        "1.50", "NaN", "-0.001",
    ]));
    let batch = RecordBatch::try_new(schema, vec![column])?;
    let text = export_one(&insert(Dialect::Postgres, "t", 10)?, &batch)?;
    assert_eq!(
        text,
        "INSERT INTO \"t\" (\"n\") VALUES\n(1.50),\n('NaN'),\n(-0.001);\n"
    );
    Ok(())
}

#[test]
fn names_with_nul_and_column_less_results_are_refused() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("out.sql");
    let batch = texts(&[Some("a")])?;
    let mut named = Exporter::create(&path, &insert(Dialect::Postgres, "bad\0name", 10)?)?;
    let error = named.begin(batch.schema()).err();
    assert!(matches!(error, Some(ExportError::InvalidName(name)) if name == "bad\0name"));
    drop(named);
    let mut exporter = Exporter::create(&path, &insert(Dialect::Postgres, "t", 10)?)?;
    let error = exporter.begin(Arc::new(Schema::empty())).err();
    assert!(matches!(error, Some(ExportError::NoColumns)));
    drop(exporter);
    assert_eq!(std::fs::read_dir(directory.path())?.count(), 0);
    Ok(())
}
