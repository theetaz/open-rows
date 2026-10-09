// SPDX-License-Identifier: Apache-2.0
//! Typed temporal and decimal results, the tagged-text fallback for zero
//! dates and out-of-range values, and edits keyed by typed columns.
use arrow_array::RecordBatch;
use arrow_array::cast::AsArray;
use arrow_array::types::{
    Date32Type, Decimal128Type, Int16Type, Time64MicrosecondType, TimestampMicrosecondType,
};
use arrow_schema::{DataType, TimeUnit};
use openrows_driver_api::metadata::{DECIMAL_EXTENSION, EXTENSION_NAME, TEXT_FALLBACK};
use openrows_driver_api::{Editor, NoticeLevel, Session};
use openrows_driver_mysql::MySqlSession;
use openrows_sql::Dialect;
use openrows_sql::edit::{CellValue, Changes, Edit, EditKind, EditTable, Values, edit_sql};
use openrows_sql::select::TableRef;

use super::support::{Collect, DATABASE, TestResult, run, session};

const MOMENTS: &[&str] = &[
    // Zero dates and zero timestamps need a permissive mode.
    "SET SESSION sql_mode = ''",
    "DROP TABLE IF EXISTS moments",
    "CREATE TABLE moments (id INT PRIMARY KEY, d DATE, dt DATETIME(6), ts TIMESTAMP(2) NULL, \
     t TIME(3), y YEAR, n DECIMAL(38,10), wide DECIMAL(65,30))",
    "INSERT INTO moments VALUES \
     (1, '2024-02-29', '0001-01-01 00:00:00.000001', '2024-05-06 07:08:09.50', '23:59:59.999', \
      2155, '-1234567890123456789012345678.0123456789', 1.5), \
     (2, '0000-00-00', '0000-00-00 00:00:00', '0000-00-00 00:00:00', '-838:59:59', 0, NULL, NULL)",
];

fn texts(batch: &RecordBatch, column: usize) -> Vec<Option<String>> {
    batch
        .column(column)
        .as_string::<i32>()
        .iter()
        .map(|value| value.map(str::to_owned))
        .collect()
}

fn owned(values: &[&str]) -> Vec<Option<String>> {
    values
        .iter()
        .map(|value| Some((*value).to_owned()))
        .collect()
}

async fn select(
    session: &mut MySqlSession,
    sql: &str,
    params: &[Option<String>],
) -> Result<Collect, Box<dyn std::error::Error>> {
    let mut sink = Collect::default();
    if params.is_empty() {
        session.execute(sql, &mut sink).await?;
    } else {
        session.execute_bound(sql, params, &mut sink).await?;
    }
    Ok(sink)
}

#[tokio::test]
#[ignore = "needs MySQL or MariaDB; run with `just test-mysql`"]
async fn typed_temporal_and_decimal_values_in_both_protocols() -> TestResult {
    let mut session = session().await?;
    run(&mut session, MOMENTS).await?;
    let sql = "SELECT d, dt, ts, t, y, n FROM moments WHERE id = 1";
    let bound = "SELECT d, dt, ts, t, y, n FROM moments WHERE id = ?";
    for (sql, params) in [(sql, vec![]), (bound, vec![Some("1".to_owned())])] {
        let sink = select(&mut session, sql, &params).await?;
        let types: Vec<DataType> = sink.schemas[0]
            .fields()
            .iter()
            .map(|field| field.data_type().clone())
            .collect();
        let micros = DataType::Timestamp(TimeUnit::Microsecond, None);
        let expected = [
            DataType::Date32,
            micros.clone(),
            micros,
            DataType::Time64(TimeUnit::Microsecond),
            DataType::Int16,
            DataType::Decimal128(38, 10),
        ];
        assert_eq!(types, expected, "params {params:?}");
        let batch = &sink.batches[0];
        assert_eq!(
            batch.column(0).as_primitive::<Date32Type>().value(0),
            19_782
        );
        let dt = batch.column(1).as_primitive::<TimestampMicrosecondType>();
        assert_eq!(dt.value(0), -62_135_596_799_999_999);
        let ts = batch.column(2).as_primitive::<TimestampMicrosecondType>();
        assert_eq!(
            ts.value(0),
            1_714_979_289_500_000,
            "session wall-clock time"
        );
        let t = batch.column(3).as_primitive::<Time64MicrosecondType>();
        assert_eq!(t.value(0), 86_399_999_000);
        assert_eq!(batch.column(4).as_primitive::<Int16Type>().value(0), 2155);
        let n = batch.column(5).as_primitive::<Decimal128Type>();
        assert_eq!(
            n.value(0),
            -12_345_678_901_234_567_890_123_456_780_123_456_789
        );
    }
    Ok(())
}

#[tokio::test]
#[ignore = "needs MySQL or MariaDB; run with `just test-mysql`"]
async fn zero_dates_and_out_of_range_values_become_tagged_text() -> TestResult {
    let mut session = session().await?;
    run(&mut session, MOMENTS).await?;
    let sql = "SELECT * FROM moments ORDER BY id";
    let bound = "SELECT * FROM moments WHERE id > ? ORDER BY id";
    for (sql, params) in [(sql, vec![]), (bound, vec![Some("0".to_owned())])] {
        let sink = select(&mut session, sql, &params).await?;
        assert_eq!(
            sink.schemas.len(),
            1,
            "the fallback happened in the first batch"
        );
        let schema = &sink.schemas[0];
        for column in 1..=4 {
            let field = schema.field(column);
            assert_eq!(field.data_type(), &DataType::Utf8, "{}", field.name());
            assert_eq!(field.metadata()[TEXT_FALLBACK], "true");
        }
        let batch = &sink.batches[0];
        assert_eq!(texts(batch, 1), owned(&["2024-02-29", "0000-00-00"]));
        let dt = ["0001-01-01 00:00:00.000001", "0000-00-00 00:00:00.000000"];
        assert_eq!(texts(batch, 2), owned(&dt), "params {params:?}");
        let ts = ["2024-05-06 07:08:09.50", "0000-00-00 00:00:00.00"];
        assert_eq!(texts(batch, 3), owned(&ts), "params {params:?}");
        assert_eq!(texts(batch, 4), owned(&["23:59:59.999", "-838:59:59.000"]));
        assert_eq!(batch.column(5).as_primitive::<Int16Type>().value(1), 0);
        assert_eq!(
            schema.field(7).metadata()[EXTENSION_NAME],
            DECIMAL_EXTENSION
        );
        let wide = texts(batch, 7);
        assert_eq!(wide[0].as_deref(), Some("1.500000000000000000000000000000"));
    }
    Ok(())
}

#[tokio::test]
#[ignore = "needs MySQL or MariaDB; run with `just test-mysql`"]
async fn a_zero_date_after_the_first_batch_continues_in_a_new_result() -> TestResult {
    let mut session = session().await?;
    run(
        &mut session,
        &[
            "SET SESSION sql_mode = ''",
            "DROP TABLE IF EXISTS days",
            "CREATE TABLE days (id INT PRIMARY KEY, d DATE)",
            "INSERT INTO days (id, d) SELECT i, IF(i = 250, '0000-00-00', '2024-01-01') FROM \
             (WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 300) \
             SELECT i FROM n) AS seq",
        ],
    )
    .await?;
    let sink = select(&mut session, "SELECT d FROM days ORDER BY id", &[]).await?;
    let types: Vec<DataType> = sink
        .schemas
        .iter()
        .map(|schema| schema.field(0).data_type().clone())
        .collect();
    assert_eq!(types, [DataType::Date32, DataType::Utf8]);
    assert_eq!(sink.rows(), 300);
    assert!(
        sink.notices.iter().any(
            |(level, message)| *level == NoticeLevel::Warning && message.contains("new result")
        ),
        "{:?}",
        sink.notices
    );
    Ok(())
}

#[tokio::test]
#[ignore = "needs MySQL or MariaDB; run with `just test-mysql`"]
async fn rows_keyed_by_temporal_and_decimal_columns_edit_and_filter() -> TestResult {
    let mut session = session().await?;
    run(
        &mut session,
        &[
            "DROP TABLE IF EXISTS stamps",
            "CREATE TABLE stamps (at DATETIME(3), amount DECIMAL(10,2), note VARCHAR(10), \
             PRIMARY KEY (at, amount))",
            "INSERT INTO stamps VALUES ('2024-05-06 07:08:09.500', 12.50, 'a')",
        ],
    )
    .await?;
    let info = session.row_edit_info(DATABASE, "stamps").await?;
    assert_eq!(info.key, ["at", "amount"]);
    // The grid shows the server's own text (fraction padded to the column's precision).
    let keys = [
        ("at".to_owned(), "2024-05-06 07:08:09.500".to_owned()),
        ("amount".to_owned(), "12.50".to_owned()),
    ];
    let original: Values = session
        .edit_snapshot((DATABASE, "stamps"), &keys)
        .await?
        .into_iter()
        .collect();
    assert_eq!(original["at"].as_deref(), Some("2024-05-06 07:08:09.500"));
    let table = EditTable {
        table: TableRef {
            schema: DATABASE.into(),
            name: "stamps".into(),
        },
        columns: vec!["at".into(), "amount".into(), "note".into()],
        key: info.key,
        protected: info.protected,
    };
    let edit = Edit {
        kind: EditKind::Update,
        original,
        values: Changes::from([
            ("note".into(), CellValue::Text("b".into())),
            ("amount".into(), CellValue::Text("13.75".into())),
        ]),
    };
    session
        .commit_edits(&[edit_sql(Dialect::MySql, &table, &edit)?])
        .await?;
    let filter = "SELECT note FROM stamps WHERE `at` = ? AND amount = ?";
    let params = [
        Some("2024-05-06 07:08:09.500".to_owned()),
        Some("13.75".to_owned()),
    ];
    let sink = select(&mut session, filter, &params).await?;
    assert_eq!(texts(&sink.batches[0], 0), owned(&["b"]));
    Ok(())
}
