// SPDX-License-Identifier: Apache-2.0
use super::*;
use arrow_array::{Array, BinaryArray, Int64Array, StringArray, UInt64Array};
use openrows_driver_api::DriverError;

mod typed;

fn column(name: &str, column_type: ColumnType) -> Column {
    Column::new(column_type)
        .with_name(name.as_bytes())
        .with_character_set(45)
}

fn bytes(text: &str) -> Vec<u8> {
    text.as_bytes().to_vec()
}

#[test]
fn column_types_map_to_arrow_with_source_types() {
    let columns = [
        column("id", ColumnType::MYSQL_TYPE_LONGLONG),
        column("big", ColumnType::MYSQL_TYPE_LONGLONG).with_flags(ColumnFlags::UNSIGNED_FLAG),
        column("price", ColumnType::MYSQL_TYPE_NEWDECIMAL),
        column("doc", ColumnType::MYSQL_TYPE_JSON),
        column("at", ColumnType::MYSQL_TYPE_DATETIME),
        column("raw", ColumnType::MYSQL_TYPE_BLOB).with_character_set(BINARY_CHARSET),
        column("note", ColumnType::MYSQL_TYPE_BLOB),
        column("mood", ColumnType::MYSQL_TYPE_STRING).with_flags(ColumnFlags::ENUM_FLAG),
        column("flag", ColumnType::MYSQL_TYPE_BIT),
    ];
    let schema = schema(&columns);
    let types: Vec<_> = schema
        .fields()
        .iter()
        .map(|f| f.data_type().clone())
        .collect();
    assert_eq!(
        types,
        vec![
            DataType::Int64,
            DataType::UInt64,
            DataType::Utf8,
            DataType::Utf8,
            DataType::Timestamp(TimeUnit::Microsecond, None),
            DataType::Binary,
            DataType::Utf8,
            DataType::Utf8,
            DataType::UInt64,
        ]
    );
    let meta = |index: usize, key: &str| schema.field(index).metadata().get(key).cloned();
    assert_eq!(meta(2, EXTENSION_NAME).as_deref(), Some(DECIMAL_EXTENSION));
    assert_eq!(meta(3, EXTENSION_NAME).as_deref(), Some(JSON_EXTENSION));
    assert_eq!(meta(4, SOURCE_TYPE).as_deref(), Some("datetime"));
    assert_eq!(meta(7, SOURCE_TYPE).as_deref(), Some("enum"));
}

#[test]
fn text_values_decode_into_typed_arrays() -> Result<(), DriverError> {
    let columns = [
        column("id", ColumnType::MYSQL_TYPE_LONG),
        column("big", ColumnType::MYSQL_TYPE_LONGLONG).with_flags(ColumnFlags::UNSIGNED_FLAG),
        column("name", ColumnType::MYSQL_TYPE_VAR_STRING),
        column("raw", ColumnType::MYSQL_TYPE_BLOB).with_character_set(BINARY_CHARSET),
        column("flag", ColumnType::MYSQL_TYPE_BIT),
    ];
    let mut builder = BatchBuilder::new(&columns);
    builder.append(&[
        Some(bytes("-7")),
        Some(bytes("18446744073709551615")),
        Some(bytes("é")),
        Some(vec![0, 255]),
        Some(vec![0x31]),
    ])?;
    builder.append(&[None, None, None, None, None])?;
    let batch = builder.finish()?;
    assert_eq!(batch.num_rows(), 2);
    let ids = batch
        .column(0)
        .as_any()
        .downcast_ref::<Int64Array>()
        .map(|a| a.value(0));
    assert_eq!(ids, Some(-7));
    let big = batch
        .column(1)
        .as_any()
        .downcast_ref::<UInt64Array>()
        .map(|a| a.value(0));
    assert_eq!(big, Some(u64::MAX));
    let name = batch
        .column(2)
        .as_any()
        .downcast_ref::<StringArray>()
        .map(|a| a.value(0).to_owned());
    assert_eq!(name.as_deref(), Some("é"));
    let raw = batch
        .column(3)
        .as_any()
        .downcast_ref::<BinaryArray>()
        .map(|a| a.value(0).to_vec());
    assert_eq!(raw, Some(vec![0, 255]));
    let flag = batch
        .column(4)
        .as_any()
        .downcast_ref::<UInt64Array>()
        .map(|a| a.value(0));
    assert_eq!(flag, Some(0x31), "BIT bytes are a number, not digits");
    assert!(batch.column(0).is_null(1));
    assert_eq!(builder.rows(), 0, "finish starts a new batch");
    Ok(())
}

#[test]
fn malformed_numbers_name_their_column() {
    let columns = [column("id", ColumnType::MYSQL_TYPE_LONG)];
    let mut builder = BatchBuilder::new(&columns);
    let error = builder.append(&[Some(bytes("12x"))]);
    assert!(matches!(error, Err(DriverError::Decode { column: 0, .. })));
}
