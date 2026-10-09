// SPDX-License-Identifier: Apache-2.0
//! Typed temporal and decimal columns and their tagged-text fallback.
use super::*;
use arrow_array::RecordBatch;
use arrow_array::cast::AsArray;
use arrow_array::types::{
    Date32Type, Decimal128Type, Int16Type, Time64MicrosecondType, TimestampMicrosecondType,
};

fn typed(name: &str, column_type: ColumnType, length: u32, decimals: u8) -> Column {
    Column::new(column_type)
        .with_name(name.as_bytes())
        .with_character_set(BINARY_CHARSET)
        .with_column_length(length)
        .with_decimals(decimals)
}

fn row(values: &[Option<&str>]) -> Vec<Option<Vec<u8>>> {
    values
        .iter()
        .map(|value| value.map(|text| text.as_bytes().to_vec()))
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

#[test]
fn temporal_decimal_and_year_columns_decode_typed() -> Result<(), DriverError> {
    let columns = [
        typed("d", ColumnType::MYSQL_TYPE_DATE, 10, 0),
        typed("at", ColumnType::MYSQL_TYPE_DATETIME, 23, 3),
        typed("ts", ColumnType::MYSQL_TYPE_TIMESTAMP, 19, 0),
        typed("t", ColumnType::MYSQL_TYPE_TIME, 17, 6),
        typed("y", ColumnType::MYSQL_TYPE_YEAR, 4, 0),
        typed("price", ColumnType::MYSQL_TYPE_NEWDECIMAL, 12, 2),
    ];
    let mut builder = BatchBuilder::new(&columns);
    builder.append(&row(&[
        Some("2024-05-06"),
        Some("2024-05-06 07:08:09.120"),
        Some("1970-01-01 00:00:01"),
        Some("23:59:59.000001"),
        Some("2155"),
        Some("-12345678.90"),
    ]))?;
    builder.append(&row(&[None; 6]))?;
    let batch = builder.finish()?;
    let schema = batch.schema();
    let date = batch.column(0).as_primitive::<Date32Type>();
    assert_eq!((date.value(0), date.is_null(1)), (19_849, true));
    let stamp = batch.column(1).as_primitive::<TimestampMicrosecondType>();
    assert_eq!(stamp.value(0), 1_714_979_289_120_000);
    let ts = batch.column(2).as_primitive::<TimestampMicrosecondType>();
    assert_eq!(ts.value(0), 1_000_000);
    let time = batch.column(3).as_primitive::<Time64MicrosecondType>();
    assert_eq!(time.value(0), 86_399_000_001);
    assert_eq!(batch.column(4).as_primitive::<Int16Type>().value(0), 2155);
    let price = batch.column(5).as_primitive::<Decimal128Type>();
    assert_eq!(price.value(0), -1_234_567_890);
    assert_eq!(schema.field(5).data_type(), &DataType::Decimal128(10, 2));
    let meta = |index: usize, key: &str| schema.field(index).metadata().get(key).cloned();
    assert_eq!(meta(1, FRACTION_DIGITS).as_deref(), Some("3"));
    assert_eq!(meta(2, SOURCE_TYPE).as_deref(), Some("timestamp"));
    assert_eq!(meta(3, FRACTION_DIGITS).as_deref(), Some("6"));
    assert_eq!(meta(5, EXTENSION_NAME), None);
    Ok(())
}

#[test]
fn decimal_precision_comes_from_length_sign_and_point() {
    let decimal = |length: u32, decimals: u8, unsigned: bool| {
        let mut column = typed("n", ColumnType::MYSQL_TYPE_NEWDECIMAL, length, decimals);
        if unsigned {
            column = column.with_flags(ColumnFlags::UNSIGNED_FLAG);
        }
        describe(&column)
    };
    let kind = |precision, scale| Kind::Decimal { precision, scale };
    assert_eq!(decimal(12, 2, false).0, kind(10, 2));
    assert_eq!(decimal(11, 0, false).0, kind(10, 0));
    assert_eq!(decimal(11, 2, true).0, kind(10, 2));
    assert_eq!(decimal(39, 0, false).0, kind(38, 0));
    assert_eq!(decimal(40, 0, false).0, Kind::Text);
    let wide = decimal(67, 2, false);
    assert_eq!((wide.0, wide.2), (Kind::Text, Some(DECIMAL_EXTENSION)));
    assert_eq!(decimal(0, 0, false).0, Kind::Text);
    let unfixed = typed("t", ColumnType::MYSQL_TYPE_DATETIME, 26, 31);
    assert_eq!(describe(&unfixed).0, Kind::DateTime { fsp: None });
    assert_eq!(
        schema(&[unfixed]).field(0).metadata().get(FRACTION_DIGITS),
        None
    );
}

#[test]
fn a_zero_date_switches_its_column_to_tagged_text() -> Result<(), DriverError> {
    let columns = [
        typed("d", ColumnType::MYSQL_TYPE_DATETIME, 23, 3),
        typed("n", ColumnType::MYSQL_TYPE_LONG, 11, 0),
    ];
    let mut builder = BatchBuilder::new(&columns);
    let before = builder.schema();
    builder.append(&row(&[Some("2024-05-06 07:08:09.500"), Some("1")]))?;
    builder.append(&row(&[None, Some("2")]))?;
    builder.append(&row(&[Some("0000-00-00 00:00:00.000"), Some("3")]))?;
    builder.append(&row(&[Some("2024-01-01 00:00:00.000"), Some("4")]))?;
    let batch = builder.finish()?;
    assert!(!Arc::ptr_eq(&before, &batch.schema()), "the schema changed");
    let field = batch.schema_ref().field(0).clone();
    assert_eq!(field.data_type(), &DataType::Utf8);
    assert_eq!(field.metadata()[TEXT_FALLBACK], "true");
    assert_eq!(field.metadata()[SOURCE_TYPE], "datetime");
    assert_eq!(field.metadata().get(FRACTION_DIGITS), None);
    assert_eq!(
        texts(&batch, 0),
        [
            Some("2024-05-06 07:08:09.500".to_owned()),
            None,
            Some("0000-00-00 00:00:00.000".to_owned()),
            Some("2024-01-01 00:00:00.000".to_owned()),
        ]
    );
    assert_eq!(
        batch.column(1).data_type(),
        &DataType::Int64,
        "others stay typed"
    );
    builder.append(&row(&[Some("2024-02-02 00:00:00.000"), Some("5")]))?;
    let next = builder.finish()?;
    assert_eq!(
        next.column(0).data_type(),
        &DataType::Utf8,
        "for the rest of the result"
    );
    Ok(())
}

#[test]
fn out_of_range_times_and_decimals_fall_back_never_corrupt() -> Result<(), DriverError> {
    let columns = [
        typed("t", ColumnType::MYSQL_TYPE_TIME, 10, 0),
        typed("p", ColumnType::MYSQL_TYPE_NEWDECIMAL, 6, 2),
        typed("d", ColumnType::MYSQL_TYPE_DATE, 10, 0),
    ];
    let mut builder = BatchBuilder::new(&columns);
    builder.append(&row(&[
        Some("12:00:00"),
        Some("999.99"),
        Some("2024-02-30"),
    ]))?;
    builder.append(&row(&[
        Some("-838:59:59"),
        Some("1000.00"),
        Some("2024-00-01"),
    ]))?;
    let batch = builder.finish()?;
    assert_eq!(
        texts(&batch, 0),
        [Some("12:00:00".to_owned()), Some("-838:59:59".to_owned())]
    );
    assert_eq!(
        texts(&batch, 1),
        [Some("999.99".to_owned()), Some("1000.00".to_owned())]
    );
    assert_eq!(
        texts(&batch, 2),
        [Some("2024-02-30".to_owned()), Some("2024-00-01".to_owned())]
    );
    Ok(())
}

#[test]
fn malformed_years_are_decode_errors() {
    let mut builder = BatchBuilder::new(&[typed("y", ColumnType::MYSQL_TYPE_YEAR, 4, 0)]);
    let error = builder.append(&row(&[Some("20x4")]));
    assert!(matches!(error, Err(DriverError::Decode { column: 0, .. })));
}
