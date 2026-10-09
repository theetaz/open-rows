// SPDX-License-Identifier: Apache-2.0
use super::support::{Collect, TestResult, config};
use arrow_array::{
    Array, ArrayRef, Date32Array, Decimal128Array, FixedSizeBinaryArray, Int8Array, Int32Array,
    Int64Array, ListArray, MapArray, StringArray, StructArray, TimestampMillisecondArray,
    UInt16Array, UInt32Array, builder::StringDictionaryBuilder, types::Int32Type,
};
use openrows_driver_api::{ConnectTrace, Session, metadata::SOURCE_TYPE};
use openrows_driver_clickhouse::connect;
use std::sync::Arc;

type Case = (&'static str, &'static str, ArrayRef);
fn cases() -> Result<Vec<Case>, Box<dyn std::error::Error + Send + Sync>> {
    let mut dictionary = StringDictionaryBuilder::<Int32Type>::new();
    dictionary.append("x")?;
    Ok(vec![
        (
            "CAST(NULL AS Nullable(Int64))",
            "Nullable(Int64)",
            Arc::new(Int64Array::from(vec![None])),
        ),
        (
            "CAST('x' AS LowCardinality(String))",
            "LowCardinality(String)",
            Arc::new(dictionary.finish()),
        ),
        (
            "CAST('123456789012345678.12' AS Decimal(20,2))",
            "Decimal(20, 2)",
            Arc::new(
                Decimal128Array::from(vec![12_345_678_901_234_567_812_i128])
                    .with_precision_and_scale(20, 2)?,
            ),
        ),
        (
            "toDate('2024-05-06')",
            "Date",
            Arc::new(UInt16Array::from(vec![19849])),
        ),
        (
            "toDate32('2024-05-06')",
            "Date32",
            Arc::new(Date32Array::from(vec![19849])),
        ),
        (
            "toDateTime64('2024-05-06 12:00:00.123',3,'UTC')",
            "DateTime64(3, 'UTC')",
            Arc::new(TimestampMillisecondArray::from(vec![1_714_996_800_123]).with_timezone("UTC")),
        ),
        (
            "toUUID('00000000-0000-0000-0000-000000000001')",
            "UUID",
            Arc::new(StringArray::from(vec![
                "00000000-0000-0000-0000-000000000001",
            ])),
        ),
        (
            "toIPv4('127.0.0.1')",
            "IPv4",
            Arc::new(UInt32Array::from(vec![2_130_706_433])),
        ),
        (
            "toIPv6('::1')",
            "IPv6",
            Arc::new(FixedSizeBinaryArray::try_from_iter(
                vec![[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]].into_iter(),
            )?),
        ),
        (
            "CAST('a' AS Enum8('a'=1,'b'=2))",
            "Enum8('a' = 1, 'b' = 2)",
            Arc::new(Int8Array::from(vec![1])),
        ),
    ])
}

#[tokio::test]
#[ignore = "needs ClickHouse; run just test-clickhouse"]
async fn native_arrow_types_and_source_metadata() -> TestResult {
    let mut session = connect(&config()?, &mut ConnectTrace::default()).await?;
    for (expression, source, expected) in cases()? {
        let mut sink = Collect::default();
        session
            .execute(&format!("SELECT {expression} AS v"), &mut sink)
            .await?;
        let schema = sink.schema.ok_or("no schema")?;
        assert_eq!(
            schema.field(0).data_type(),
            expected.data_type(),
            "{source}"
        );
        assert_eq!(
            schema
                .field(0)
                .metadata()
                .get(SOURCE_TYPE)
                .map(String::as_str),
            Some(source)
        );
        let batch = sink.batches.first().ok_or("no batch")?;
        assert_eq!(batch.column(0).to_data(), expected.to_data(), "{source}");
    }
    nested_values(&mut session).await
}

async fn nested_values(session: &mut dyn Session) -> TestResult {
    let mut sink = Collect::default();
    session.execute("SELECT [toInt32(1),toInt32(2)] AS a, map('a', toInt32(1)) AS m, tuple(toInt32(1), 'x') AS t", &mut sink).await?;
    let batch = sink.batches.first().ok_or("no batch")?;
    let list = batch
        .column(0)
        .as_any()
        .downcast_ref::<ListArray>()
        .ok_or("not a list")?;
    assert_eq!(
        list.value(0).to_data(),
        Int32Array::from(vec![1, 2]).to_data()
    );
    let map = batch
        .column(1)
        .as_any()
        .downcast_ref::<MapArray>()
        .ok_or("not a map")?;
    assert_eq!(map.keys().to_data(), StringArray::from(vec!["a"]).to_data());
    assert_eq!(map.values().to_data(), Int32Array::from(vec![1]).to_data());
    let tuple = batch
        .column(2)
        .as_any()
        .downcast_ref::<StructArray>()
        .ok_or("not a tuple")?;
    assert_eq!(
        tuple.column(0).to_data(),
        Int32Array::from(vec![1]).to_data()
    );
    assert_eq!(
        tuple.column(1).to_data(),
        StringArray::from(vec!["x"]).to_data()
    );
    Ok(())
}

#[tokio::test]
#[ignore = "needs ClickHouse; run just test-clickhouse"]
async fn high_precision_and_nested_uuid_values_remain_exact() -> TestResult {
    use arrow_array::{cast::AsArray, types::Decimal256Type};
    let mut session = connect(&config()?, &mut ConnectTrace::default()).await?;
    let mut sink = Collect::default();
    session.execute("SELECT CAST('1234567890123456789012345678901234567890.1234567890' AS Decimal(50,10)) AS d, toDateTime64('2024-05-06 12:00:00.123456789',9,'UTC') AS t, [toUUID('00000000-0000-0000-0000-000000000001')] AS u, CAST('b' AS Enum16('a'=1,'b'=300)) AS e", &mut sink).await?;
    let batch = sink.batches.first().ok_or("no batch")?;
    assert_eq!(
        batch
            .column(0)
            .as_primitive::<Decimal256Type>()
            .value_as_string(0),
        "1234567890123456789012345678901234567890.1234567890"
    );
    assert_eq!(
        batch
            .column(1)
            .as_primitive::<arrow_array::types::TimestampNanosecondType>()
            .value(0),
        1_714_996_800_123_456_789
    );
    let values = batch.column(2).as_list::<i32>().value(0);
    assert_eq!(
        values.as_string::<i32>().value(0),
        "00000000-0000-0000-0000-000000000001"
    );
    assert_eq!(
        batch
            .column(3)
            .as_primitive::<arrow_array::types::Int16Type>()
            .value(0),
        300
    );
    assert_eq!(
        batch
            .schema()
            .field(2)
            .metadata()
            .get(SOURCE_TYPE)
            .map(String::as_str),
        Some("Array(UUID)")
    );
    Ok(())
}
