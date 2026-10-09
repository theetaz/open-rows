// SPDX-License-Identifier: Apache-2.0
use super::*;

fn desc(type_oid: u32, type_modifier: i32) -> ColumnDesc {
    ColumnDesc {
        name: "c".to_owned(),
        type_oid,
        type_modifier,
    }
}

fn typmod(precision: i32, scale: i32) -> i32 {
    ((precision << 16) | scale) + VARHDRSZ
}

#[test]
fn numeric_fits_decimal128_only_with_known_precision_up_to_38() {
    let kind = |modifier| ColumnKind::of(&desc(NUMERIC_OID, modifier));
    assert_eq!(kind(-1), ColumnKind::NumericText);
    assert_eq!(kind(typmod(10, 2)).data_type(), DataType::Decimal128(10, 2));
    assert_eq!(
        kind(typmod(38, 38)).data_type(),
        DataType::Decimal128(38, 38)
    );
    assert_eq!(kind(typmod(39, 2)), ColumnKind::NumericText);
    assert_eq!(kind(typmod(1000, 0)), ColumnKind::NumericText);
}

#[test]
fn unknown_types_use_text_format_and_carry_their_name() {
    let point = desc(600, -1);
    let names = HashMap::from([(600, "point".to_owned())]);
    let kind = ColumnKind::of(&point);
    assert_eq!((kind, kind.format_code()), (ColumnKind::Unknown, 0));
    assert!(!needs_name(&point, &names));
    assert!(needs_name(&point, &HashMap::new()));
    let field = field(&point, kind, &names);
    assert_eq!(field.data_type(), &DataType::Utf8);
    assert_eq!(field.metadata()[SOURCE_TYPE], "point");
}

#[test]
fn extension_types_are_tagged() {
    let names = HashMap::new();
    let cases = [
        (2950, UUID_EXTENSION),
        (3802, JSON_EXTENSION),
        (869, INET_EXTENSION),
        (650, INET_EXTENSION),
        (829, MACADDR_EXTENSION),
        (3904, RANGE_EXTENSION),
        (4536, RANGE_EXTENSION),
    ];
    for (oid, extension) in cases {
        let column = desc(oid, -1);
        let field = field(&column, ColumnKind::of(&column), &names);
        assert_eq!(field.metadata()[EXTENSION_NAME], extension, "oid {oid}");
        assert!(!needs_name(&column, &names), "oid {oid}");
    }
    let numeric = desc(NUMERIC_OID, -1);
    let field = field(&numeric, ColumnKind::of(&numeric), &names);
    assert_eq!(field.metadata()[EXTENSION_NAME], DECIMAL_EXTENSION);
    assert_eq!(field.metadata()[SOURCE_TYPE], "numeric");
}

#[test]
fn temporal_types_map_to_arrow_time_and_interval() {
    let kind = |oid| ColumnKind::of(&desc(oid, -1));
    assert_eq!(
        kind(1083).data_type(),
        DataType::Time64(TimeUnit::Microsecond)
    );
    assert_eq!(
        kind(1186).data_type(),
        DataType::Interval(IntervalUnit::MonthDayNano)
    );
    assert_eq!(kind(1266).data_type(), DataType::Utf8);
    for oid in [1083, 1186, 1266, 869, 829] {
        assert_eq!(kind(oid).format_code(), 1, "oid {oid}");
    }
    assert_eq!(kind(3910).format_code(), 0);
}

fn list_item(data_type: &DataType) -> Option<&Field> {
    match data_type {
        DataType::List(item) => Some(item),
        _ => None,
    }
}

#[test]
fn supported_arrays_are_lists_of_their_element_type() {
    let names = HashMap::new();
    let int4s = desc(1007, -1);
    let kind = ColumnKind::of(&int4s);
    assert_eq!(kind.element(), Some(ColumnKind::Int4));
    assert_eq!(kind.format_code(), 1);
    assert!(needs_name(&int4s, &names));
    let lookup = HashMap::from([(1007, "integer[]".to_owned())]);
    let field = field(&int4s, kind, &lookup);
    assert_eq!(field.metadata()[SOURCE_TYPE], "integer[]");
    let item = list_item(field.data_type()).expect("a list");
    assert_eq!(item.data_type(), &DataType::Int32);
    assert_eq!(item.metadata()[SOURCE_TYPE], "int4");
    let uuids = ColumnKind::of(&desc(2951, -1)).data_type();
    let item = list_item(&uuids).expect("a list");
    assert_eq!(item.metadata()[EXTENSION_NAME], UUID_EXTENSION);
    // numeric(10, 2)[] keeps the element typmod.
    let decimals = ColumnKind::of(&desc(1231, typmod(10, 2)));
    assert_eq!(
        decimals.element().map(ColumnKind::data_type),
        Some(DataType::Decimal128(10, 2))
    );
    // int4range[] is not decoded natively.
    assert_eq!(ColumnKind::of(&desc(3905, -1)), ColumnKind::Unknown);
}
