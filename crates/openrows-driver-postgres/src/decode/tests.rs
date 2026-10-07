// SPDX-License-Identifier: Apache-2.0
use super::ResultDecoder;
use crate::fixture::{self, FixtureType};
use arrow_array::cast::AsArray;
use arrow_array::types::{
    Date32Type, Decimal128Type, Float32Type, Float64Type, Int16Type, Int32Type, Int64Type,
    TimestampMicrosecondType,
};
use arrow_array::{Array, RecordBatch};
use arrow_schema::DataType;
use openrows_driver_api::{BatchPolicy, DriverError};
use postgres_protocol::message::backend::Message;
use std::collections::HashMap;

fn decode_all(
    fixture: &fixture::Fixture,
    policy: BatchPolicy,
) -> Result<Vec<RecordBatch>, DriverError> {
    let mut decoder = ResultDecoder::new(&fixture.columns, &HashMap::new(), policy);
    let mut frames = fixture.frames.clone();
    let mut batches = Vec::new();
    while let Some(Message::DataRow(body)) = Message::parse(&mut frames)? {
        batches.extend(decoder.decode_row(body.buffer())?);
    }
    batches.extend(decoder.finish()?);
    Ok(batches)
}

fn numeric_text(unscaled: i64) -> String {
    let sign = if unscaled < 0 { "-" } else { "" };
    let magnitude = unscaled.unsigned_abs();
    format!("{sign}{}.{:04}", magnitude / 10_000, magnitude % 10_000)
}

/// Expected value of fixture cell (row, column) rendered for comparison.
fn expected(kind: FixtureType, n: u64) -> String {
    let signed = i64::try_from(n).unwrap_or_default();
    match kind {
        FixtureType::Int2 => (i64::try_from(n % 32_768).unwrap_or_default() - 16_384).to_string(),
        FixtureType::Int4 => (signed - 500_000).to_string(),
        FixtureType::Int8 => (signed * 1_000_000_007 - 3).to_string(),
        FixtureType::Float4 => format!("{}", f64::from(u32::try_from(n).unwrap_or_default()) / 8.0),
        FixtureType::Float8 => format!(
            "{}",
            f64::from(u32::try_from(n).unwrap_or_default()) / 1024.0
        ),
        FixtureType::Bool => n.is_multiple_of(2).to_string(),
        FixtureType::Text => fixture::text(n),
        FixtureType::Bytea | FixtureType::Uuid => format!("{n:08x}"),
        FixtureType::Timestamp | FixtureType::TimestampTz => {
            (signed * 1_000_003 + 946_684_800_000_000).to_string()
        }
        FixtureType::Date => {
            (i64::try_from(n % 20_000).unwrap_or_default() - 10_000 + 10_957).to_string()
        }
        FixtureType::Decimal => (signed * 10_007 - 5_000_000_000).to_string(),
        FixtureType::Numeric => numeric_text(signed * 10_007 - 5_000_000_000),
        FixtureType::Jsonb => format!("{{\"n\": {n}}}"),
    }
}

fn actual(array: &dyn Array, row: usize) -> String {
    match array.data_type() {
        DataType::Int16 => array.as_primitive::<Int16Type>().value(row).to_string(),
        DataType::Int32 => array.as_primitive::<Int32Type>().value(row).to_string(),
        DataType::Int64 => array.as_primitive::<Int64Type>().value(row).to_string(),
        DataType::Float32 => format!(
            "{}",
            f64::from(array.as_primitive::<Float32Type>().value(row))
        ),
        DataType::Float64 => format!("{}", array.as_primitive::<Float64Type>().value(row)),
        DataType::Boolean => array.as_boolean().value(row).to_string(),
        DataType::Utf8 => array.as_string::<i32>().value(row).to_owned(),
        DataType::Binary => hex_prefix(array.as_binary::<i32>().value(row)),
        DataType::FixedSizeBinary(_) => hex_prefix(array.as_fixed_size_binary().value(row)),
        DataType::Timestamp(..) => array
            .as_primitive::<TimestampMicrosecondType>()
            .value(row)
            .to_string(),
        DataType::Date32 => array.as_primitive::<Date32Type>().value(row).to_string(),
        DataType::Decimal128(..) => array
            .as_primitive::<Decimal128Type>()
            .value(row)
            .to_string(),
        other => format!("unexpected {other}"),
    }
}

fn hex_prefix(bytes: &[u8]) -> String {
    let prefix = bytes.first_chunk::<4>().copied().unwrap_or_default();
    format!("{:08x}", u32::from_be_bytes(prefix))
}

#[test]
fn every_fixture_type_decodes_to_its_value_or_null() -> Result<(), DriverError> {
    let fixture = fixture::generate(&FixtureType::ALL, 300);
    let batches = decode_all(&fixture, BatchPolicy::default())?;
    let rows: Vec<_> = batches.iter().map(RecordBatch::num_rows).collect();
    assert_eq!(rows, [200, 100]);
    for (column, &kind) in FixtureType::ALL.iter().enumerate() {
        for row in 0..300 {
            let (batch, offset) = if row < 200 {
                (&batches[0], row)
            } else {
                (&batches[1], row - 200)
            };
            let array = batch.column(column);
            if fixture::is_null(row, column) {
                assert!(array.is_null(offset), "{kind:?} row {row} must be NULL");
            } else {
                assert_eq!(
                    actual(array, offset),
                    expected(kind, fixture::base(row, column)),
                    "{kind:?} row {row}"
                );
            }
        }
    }
    Ok(())
}

#[test]
fn batches_follow_the_policy() -> Result<(), DriverError> {
    let fixture = fixture::generate(&[FixtureType::Int4], 70_000);
    let rows: Vec<_> = decode_all(&fixture, BatchPolicy::default())?
        .iter()
        .map(RecordBatch::num_rows)
        .collect();
    assert_eq!(rows, [200, 65_536, 4_264]);
    let tight = BatchPolicy {
        max_bytes: 1_000,
        ..BatchPolicy::default()
    };
    let rows: Vec<_> = decode_all(&fixture::generate(&[FixtureType::Int4], 300), tight)?
        .iter()
        .map(RecordBatch::num_rows)
        .collect();
    // Int4 cells are 8 wire bytes (length + value), NULL cells 4: the 1000-byte
    // budget flushes after 135 rows, before the 200-row first-batch limit.
    assert_eq!(rows, [135, 135, 30]);
    Ok(())
}

#[test]
fn schema_carries_types_and_source_metadata() {
    let fixture = fixture::generate(&FixtureType::ALL, 0);
    let decoder = ResultDecoder::new(&fixture.columns, &HashMap::new(), BatchPolicy::default());
    let schema = decoder.schema();
    assert_eq!(schema.field(12).data_type(), &DataType::Decimal128(18, 4));
    assert_eq!(
        schema.field(9).data_type().to_string(),
        "Timestamp(µs, \"UTC\")"
    );
    assert!(
        schema
            .fields()
            .iter()
            .all(|f| f.metadata().contains_key("openrows.source_type"))
    );
    assert!(decoder.result_formats().iter().all(|&code| code == 1));
}

fn decode_one(fixture: &fixture::Fixture, row: &[u8]) -> Result<(), DriverError> {
    let mut decoder = ResultDecoder::new(&fixture.columns, &HashMap::new(), BatchPolicy::default());
    decoder.decode_row(row).map(|_| ())
}

#[test]
fn malformed_rows_are_typed_errors() {
    let fixture = fixture::generate(&[FixtureType::Int4, FixtureType::Text], 0);
    let short_int = [0, 0, 0, 2, 1, 2, 255, 255, 255, 255];
    let truncated = [255, 255, 255, 255, 0, 0, 0, 9, b'a'];
    let extra = [255, 255, 255, 255, 255, 255, 255, 255, 0];
    let invalid_utf8 = [255, 255, 255, 255, 0, 0, 0, 1, 0xff];
    let error = |row: &[u8]| decode_one(&fixture, row).map_err(|e| e.to_string());
    assert_eq!(
        error(&short_int),
        Err("column 0: expected 4 bytes, got 2".to_owned())
    );
    assert_eq!(
        error(&truncated),
        Err("column 1: truncated value".to_owned())
    );
    assert!(matches!(
        decode_one(&fixture, &extra),
        Err(DriverError::Protocol(_))
    ));
    assert_eq!(
        error(&invalid_utf8),
        Err("column 1: invalid UTF-8 text".to_owned())
    );
}

#[test]
fn zero_column_results_still_count_rows() -> Result<(), DriverError> {
    let mut decoder = ResultDecoder::new(&[], &HashMap::new(), BatchPolicy::default());
    for _ in 0..3 {
        assert!(decoder.decode_row(&[])?.is_none());
    }
    let batch = decoder.finish()?.map(|b| b.num_rows());
    assert_eq!(batch, Some(3));
    Ok(())
}

fn wire_row(cells: &[Option<&[u8]>]) -> Vec<u8> {
    let mut row = Vec::new();
    for cell in cells {
        match cell {
            Some(bytes) => {
                let length = i32::try_from(bytes.len()).unwrap_or(i32::MAX);
                row.extend_from_slice(&length.to_be_bytes());
                row.extend_from_slice(bytes);
            }
            None => row.extend_from_slice(&(-1_i32).to_be_bytes()),
        }
    }
    row
}

fn columns(specs: &[(&str, u32, i32)]) -> Vec<crate::types::ColumnDesc> {
    specs
        .iter()
        .map(
            |&(name, type_oid, type_modifier)| crate::types::ColumnDesc {
                name: name.to_owned(),
                type_oid,
                type_modifier,
            },
        )
        .collect()
}

fn texts(batch: &RecordBatch, column: usize) -> Vec<Option<String>> {
    batch
        .column(column)
        .as_string::<i32>()
        .iter()
        .map(|value| value.map(str::to_owned))
        .collect()
}

fn missing() -> DriverError {
    DriverError::Protocol("expected a batch".to_owned())
}

#[test]
fn unrepresentable_values_switch_their_column_to_tagged_text() -> Result<(), DriverError> {
    // timestamp, numeric(10, 2), int4
    let desc = columns(&[
        ("ts", 1114, -1),
        ("n", 1700, (10 << 16 | 2) + 4),
        ("i", 23, -1),
    ]);
    let mut decoder = ResultDecoder::new(&desc, &HashMap::new(), BatchPolicy::default());
    let twelve = [0, 1, 0, 0, 0, 0, 0, 2, 0, 12];
    let nan = [0, 0, 0, 0, 0xc0, 0, 0, 0];
    let (epoch, one) = (0_i64.to_be_bytes(), 1_i32.to_be_bytes());
    decoder.decode_row(&wire_row(&[Some(&epoch), Some(&twelve), Some(&one)]))?;
    decoder.decode_row(&wire_row(&[Some(&i64::MAX.to_be_bytes()), None, None]))?;
    decoder.decode_row(&wire_row(&[None, Some(&nan), Some(&one)]))?;
    let batch = decoder.finish()?.ok_or_else(missing)?;
    let schema = batch.schema();
    assert_eq!(schema.field(0).data_type(), &DataType::Utf8);
    assert_eq!(schema.field(1).data_type(), &DataType::Utf8);
    assert_eq!(schema.field(2).data_type(), &DataType::Int32);
    for field in &schema.fields()[..2] {
        assert_eq!(field.metadata()["openrows.text_fallback"], "true");
    }
    assert_eq!(
        schema.field(0).metadata()["openrows.source_type"],
        "timestamp"
    );
    assert_eq!(
        schema.field(1).metadata()["openrows.source_type"],
        "numeric"
    );
    let timestamps = [
        Some("2000-01-01 00:00:00".into()),
        Some("infinity".into()),
        None,
    ];
    assert_eq!(texts(&batch, 0), timestamps);
    assert_eq!(
        texts(&batch, 1),
        [Some("12.00".into()), None, Some("NaN".into())]
    );
    Ok(())
}

#[test]
fn a_late_fallback_changes_the_schema_of_later_batches_only() -> Result<(), DriverError> {
    let desc = columns(&[("d", 1082, -1)]);
    let policy = BatchPolicy {
        first_batch_rows: 1,
        ..BatchPolicy::default()
    };
    let mut decoder = ResultDecoder::new(&desc, &HashMap::new(), policy);
    let typed = decoder.decode_row(&wire_row(&[Some(&0_i32.to_be_bytes())]))?;
    let typed = typed.ok_or_else(missing)?;
    assert_eq!(typed.schema().field(0).data_type(), &DataType::Date32);
    decoder.decode_row(&wire_row(&[Some(&i32::MIN.to_be_bytes())]))?;
    let text = decoder.finish()?.ok_or_else(missing)?;
    assert_eq!(text.schema(), decoder.schema());
    assert_ne!(text.schema(), typed.schema());
    assert_eq!(texts(&text, 0), [Some("-infinity".to_owned())]);
    Ok(())
}
