// SPDX-License-Identifier: Apache-2.0
//! Every common type decodes to its documented Arrow type and exact value,
//! with `openrows.source_type`, and the tagged-text fallback.
use arrow_array::cast::AsArray;
use arrow_array::types::{
    Date32Type, Decimal128Type, Float32Type, Float64Type, Int16Type, Int32Type, Int64Type,
    Time64MicrosecondType, Time64NanosecondType, TimestampMicrosecondType, TimestampNanosecondType,
};
use arrow_array::{Array, RecordBatch};
use arrow_schema::{DataType, TimeUnit};
use openrows_driver_api::Session;
use openrows_driver_api::metadata::{
    EXTENSION_NAME, FRACTION_DIGITS, SOURCE_TYPE, TEXT_FALLBACK, UUID_EXTENSION,
};

use super::support::{Collect, TestResult, session};

const TYPES_SQL: &str = "SELECT \
    CAST(255 AS tinyint) AS tiny, CAST(-32768 AS smallint) AS small, \
    CAST(2147483647 AS int) AS i, CAST(-9007199254740993 AS bigint) AS big, \
    CAST(1 AS bit) AS b, CAST(1.5 AS real) AS r, CAST(0.1 AS float) AS f, \
    CAST(12.50 AS decimal(10, 2)) AS d, CAST(-12345678901234567890.123456789 AS numeric(38, 9)) AS n, \
    CAST(12.3456 AS money) AS m, CAST(-1.5 AS smallmoney) AS sm, \
    CAST('2024-05-06' AS date) AS dt, CAST('13:05:00.25' AS time(2)) AS t2, \
    CAST('13:05:00.1234567' AS time) AS t7, \
    CAST('2024-05-06 13:05:00.123456' AS datetime2(6)) AS dt6, \
    CAST('2024-05-06 13:05:00.1234567' AS datetime2) AS dt7, \
    CAST('2024-05-06 13:05:00.007' AS datetime) AS legacy, \
    CAST('2024-05-06 13:05:29' AS smalldatetime) AS small_legacy, \
    CAST('2024-05-06 13:05:00.5 +05:30' AS datetimeoffset(1)) AS dto, \
    CAST('00112233-4455-6677-8899-AABBCCDDEEFF' AS uniqueidentifier) AS g, \
    CAST('caf\u{e9}' AS varchar(10)) AS vc, CAST('ab' AS char(3)) AS ch, \
    N'h\u{e9}llo \u{4e16}\u{754c}' AS nv, CAST(N'x' AS nchar(2)) AS nc, \
    CAST(REPLICATE(CAST('z' AS varchar(max)), 9000) AS varchar(max)) AS big_text, \
    CAST(0x00FF10 AS varbinary(10)) AS vb, CAST(0xAB AS binary(2)) AS bn, \
    CAST('<a>1</a>' AS xml) AS x, CAST(N'{\"a\":1}' AS nvarchar(max)) AS js, \
    CAST(42 AS sql_variant) AS v, CAST(NULL AS int) AS nothing";

fn field_type(batch: &RecordBatch, name: &str) -> DataType {
    batch
        .schema()
        .field_with_name(name)
        .map(|field| field.data_type().clone())
        .unwrap_or(DataType::Null)
}

fn metadata(batch: &RecordBatch, name: &str, key: &str) -> Option<String> {
    let schema = batch.schema();
    schema
        .field_with_name(name)
        .ok()?
        .metadata()
        .get(key)
        .cloned()
}

fn column<'a>(batch: &'a RecordBatch, name: &str) -> &'a dyn Array {
    let index = batch.schema().index_of(name).unwrap_or(0);
    batch.column(index).as_ref()
}

#[tokio::test]
#[ignore = "needs SQL Server; run with `just test-mssql`"]
async fn common_types_decode_exactly() -> TestResult {
    let mut session = session().await?;
    let mut sink = Collect::default();
    session.execute(TYPES_SQL, &mut sink).await?;
    let batch = &sink.batches[0];
    for field in batch.schema().fields() {
        assert!(
            field.metadata().contains_key(SOURCE_TYPE),
            "{}",
            field.name()
        );
    }
    assert_eq!(
        column(batch, "tiny").as_primitive::<Int16Type>().value(0),
        255
    );
    assert_eq!(
        column(batch, "small").as_primitive::<Int16Type>().value(0),
        -32_768
    );
    assert_eq!(
        column(batch, "i").as_primitive::<Int32Type>().value(0),
        i32::MAX
    );
    assert_eq!(
        column(batch, "big").as_primitive::<Int64Type>().value(0),
        -9_007_199_254_740_993
    );
    assert!(column(batch, "b").as_boolean().value(0));
    assert!((column(batch, "r").as_primitive::<Float32Type>().value(0) - 1.5).abs() < f32::EPSILON);
    assert!((column(batch, "f").as_primitive::<Float64Type>().value(0) - 0.1).abs() < f64::EPSILON);
    assert_eq!(field_type(batch, "d"), DataType::Decimal128(10, 2));
    assert_eq!(
        column(batch, "d")
            .as_primitive::<Decimal128Type>()
            .value_as_string(0),
        "12.50"
    );
    assert_eq!(
        column(batch, "n")
            .as_primitive::<Decimal128Type>()
            .value_as_string(0),
        "-12345678901234567890.123456789"
    );
    assert_eq!(field_type(batch, "m"), DataType::Decimal128(19, 4));
    assert_eq!(
        column(batch, "m")
            .as_primitive::<Decimal128Type>()
            .value_as_string(0),
        "12.3456"
    );
    assert_eq!(
        column(batch, "sm")
            .as_primitive::<Decimal128Type>()
            .value_as_string(0),
        "-1.5000"
    );
    assert_eq!(
        column(batch, "dt").as_primitive::<Date32Type>().value(0),
        19_849
    );
    temporal(batch);
    text_and_binary(batch);
    Ok(())
}

fn temporal(batch: &RecordBatch) {
    let micros = |h: i64, m: i64, s: i64| ((h * 60 + m) * 60 + s) * 1_000_000;
    assert_eq!(
        column(batch, "t2")
            .as_primitive::<Time64MicrosecondType>()
            .value(0),
        micros(13, 5, 0) + 250_000
    );
    assert_eq!(metadata(batch, "t2", FRACTION_DIGITS).as_deref(), Some("2"));
    assert_eq!(
        column(batch, "t7")
            .as_primitive::<Time64NanosecondType>()
            .value(0),
        micros(13, 5, 0) * 1_000 + 123_456_700
    );
    let day = 19_849 * 86_400 * 1_000_000_i64;
    assert_eq!(
        column(batch, "dt6")
            .as_primitive::<TimestampMicrosecondType>()
            .value(0),
        day + micros(13, 5, 0) + 123_456
    );
    assert_eq!(
        field_type(batch, "dt7"),
        DataType::Timestamp(TimeUnit::Nanosecond, None)
    );
    assert_eq!(
        column(batch, "dt7")
            .as_primitive::<TimestampNanosecondType>()
            .value(0),
        (day + micros(13, 5, 0)) * 1_000 + 123_456_700
    );
    assert_eq!(
        column(batch, "legacy")
            .as_primitive::<TimestampMicrosecondType>()
            .value(0),
        day + micros(13, 5, 0) + 7_000
    );
    assert_eq!(
        metadata(batch, "legacy", FRACTION_DIGITS).as_deref(),
        Some("3")
    );
    assert_eq!(
        column(batch, "small_legacy")
            .as_primitive::<TimestampMicrosecondType>()
            .value(0),
        day + micros(13, 5, 0)
    );
    assert_eq!(
        column(batch, "dto").as_string::<i32>().value(0),
        "2024-05-06 13:05:00.5 +05:30"
    );
}

fn text_and_binary(batch: &RecordBatch) {
    assert_eq!(
        metadata(batch, "g", EXTENSION_NAME).as_deref(),
        Some(UUID_EXTENSION)
    );
    assert_eq!(
        column(batch, "g").as_fixed_size_binary().value(0),
        [
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD,
            0xEE, 0xFF
        ]
    );
    let text = |name: &str| column(batch, name).as_string::<i32>().value(0).to_owned();
    assert_eq!(text("vc"), "caf\u{e9}");
    assert_eq!(text("ch"), "ab ");
    assert_eq!(text("nv"), "h\u{e9}llo \u{4e16}\u{754c}");
    assert_eq!(text("nc"), "x ");
    assert_eq!(text("big_text").len(), 9_000);
    assert_eq!(text("x"), "<a>1</a>");
    assert_eq!(text("js"), "{\"a\":1}");
    assert_eq!(text("v"), "42");
    assert_eq!(metadata(batch, "v", TEXT_FALLBACK).as_deref(), Some("true"));
    assert_eq!(
        metadata(batch, "v", SOURCE_TYPE).as_deref(),
        Some("sql_variant")
    );
    assert_eq!(
        column(batch, "vb").as_binary::<i32>().value(0),
        [0x00, 0xFF, 0x10]
    );
    assert_eq!(
        column(batch, "bn").as_binary::<i32>().value(0),
        [0xAB, 0x00]
    );
    assert!(column(batch, "nothing").is_null(0));
}

#[tokio::test]
#[ignore = "needs SQL Server; run with `just test-mssql`"]
async fn out_of_range_datetime2_falls_back_to_tagged_text() -> TestResult {
    let mut session = session().await?;
    let mut sink = Collect::default();
    let sql = "SELECT v FROM (VALUES (CAST('2024-01-01 00:00:00.1234567' AS datetime2)), \
               (CAST('9999-12-31 23:59:59.9999999' AS datetime2))) AS t(v)";
    session.execute(sql, &mut sink).await?;
    assert_eq!(sink.schemas.len(), 1);
    let batch = &sink.batches[0];
    assert_eq!(field_type(batch, "v"), DataType::Utf8);
    assert_eq!(metadata(batch, "v", TEXT_FALLBACK).as_deref(), Some("true"));
    let values = column(batch, "v").as_string::<i32>();
    assert_eq!(values.value(0), "2024-01-01 00:00:00.1234567");
    assert_eq!(values.value(1), "9999-12-31 23:59:59.9999999");
    Ok(())
}
