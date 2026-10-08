// SPDX-License-Identifier: Apache-2.0
use super::*;

fn column(column_type: ColumnType, decimals: u8) -> Column {
    Column::new(column_type).with_decimals(decimals)
}

#[test]
fn text_rows_split_length_encoded_values_and_nulls() -> Result<(), DriverError> {
    let packet = [1, b'7', 0xfb, 3, b'a', b'b', b'c', 0];
    let values = text_row(&packet, 4)?;
    assert_eq!(
        values,
        vec![
            Some(b"7".to_vec()),
            None,
            Some(b"abc".to_vec()),
            Some(Vec::new())
        ]
    );
    assert!(
        text_row(&[5, b'x'], 1).is_err(),
        "a short value is a protocol error"
    );
    let (big, rest) = lenenc(&[0xfc, 0x10, 0x01, 9])?;
    assert_eq!((big, rest), (0x0110, &[9_u8][..]));
    Ok(())
}

#[test]
fn binary_values_render_like_the_text_protocol() {
    let datetime = column(ColumnType::MYSQL_TYPE_DATETIME, 3);
    let rendered = text_form(Value::Date(2024, 5, 6, 7, 8, 9, 123_456), &datetime);
    assert_eq!(rendered.as_deref(), Some(&b"2024-05-06 07:08:09.123"[..]));
    let date = column(ColumnType::MYSQL_TYPE_DATE, 0);
    let rendered = text_form(Value::Date(2024, 5, 6, 0, 0, 0, 0), &date);
    assert_eq!(rendered.as_deref(), Some(&b"2024-05-06"[..]));
    let time = column(ColumnType::MYSQL_TYPE_TIME, 0);
    let rendered = text_form(Value::Time(true, 1, 2, 3, 4, 0), &time);
    assert_eq!(rendered.as_deref(), Some(&b"-26:03:04"[..]));
    let int = column(ColumnType::MYSQL_TYPE_LONGLONG, 0);
    assert_eq!(
        text_form(Value::UInt(u64::MAX), &int).as_deref(),
        Some(&b"18446744073709551615"[..])
    );
    assert_eq!(text_form(Value::NULL, &int), None);
    assert_eq!(fraction(5, 6), ".000005");
}

#[test]
fn binary_rows_honour_the_null_bitmap() -> Result<(), DriverError> {
    let columns = [
        column(ColumnType::MYSQL_TYPE_LONG, 0),
        column(ColumnType::MYSQL_TYPE_VAR_STRING, 0),
    ];
    // header, bitmap (column 1 NULL: bit 1 + offset 2 = bit 3), LONG 42.
    let packet = [0x00, 0b0000_1000, 42, 0, 0, 0];
    let values = binary_row(&packet, &columns)?;
    assert_eq!(values, vec![Some(b"42".to_vec()), None]);
    Ok(())
}
