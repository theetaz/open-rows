// SPDX-License-Identifier: Apache-2.0
use super::*;
use crate::tds::types::{Layout, TypeInfo};

fn column(name: &str, sql: SqlType, user_type: u32) -> Column {
    Column {
        name: name.to_owned(),
        info: TypeInfo {
            sql,
            layout: Layout::ByteLen,
        },
        user_type,
    }
}

#[test]
fn schemas_carry_types_and_metadata() {
    let columns = [
        column("t", SqlType::Time { scale: 7 }, 0),
        column("d", SqlType::DateTime, 0),
        column("g", SqlType::Guid, 0),
        column("v", SqlType::Variant, 0),
        column("rv", SqlType::Binary { varying: false }, ROWVERSION),
        column(
            "n",
            SqlType::Decimal {
                precision: 10,
                scale: 2,
                numeric: true,
            },
            0,
        ),
    ];
    let schema = schema(&columns);
    let meta = |index: usize, key: &str| schema.field(index).metadata().get(key).cloned();
    assert_eq!(
        schema.field(0).data_type(),
        &DataType::Time64(TimeUnit::Nanosecond)
    );
    assert_eq!(meta(0, FRACTION_DIGITS).as_deref(), Some("7"));
    assert_eq!(meta(1, FRACTION_DIGITS).as_deref(), Some("3"));
    assert_eq!(meta(2, EXTENSION_NAME).as_deref(), Some(UUID_EXTENSION));
    assert_eq!(meta(3, TEXT_FALLBACK).as_deref(), Some("true"));
    assert_eq!(meta(4, SOURCE_TYPE).as_deref(), Some("timestamp"));
    assert_eq!(meta(5, SOURCE_TYPE).as_deref(), Some("numeric"));
    assert_eq!(schema.field(5).data_type(), &DataType::Decimal128(10, 2));
    let fallback = tagged_text(schema.field(0));
    assert_eq!(fallback.data_type(), &DataType::Utf8);
    assert!(!fallback.metadata().contains_key(FRACTION_DIGITS));
}

#[test]
fn sql_types_map_to_kinds() {
    assert_eq!(kind(&SqlType::Xml), Kind::Utf16);
    assert_eq!(
        kind(&SqlType::Udt {
            name: "geography".into()
        }),
        Kind::Binary
    );
    assert_eq!(
        data_type(Kind::DateTime2 { scale: 3 }),
        DataType::Timestamp(TimeUnit::Microsecond, None)
    );
    assert_eq!(data_type(Kind::Money), DataType::Decimal128(19, 4));
    assert_eq!(
        CellError::Unrepresentable("x").to_string(),
        "x value has no exact Arrow form"
    );
}
