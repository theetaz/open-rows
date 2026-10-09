// SPDX-License-Identifier: Apache-2.0
use super::super::ResultDecoder;
use super::*;
use crate::types::ColumnDesc;
use arrow_array::RecordBatch;
use arrow_array::types::Int32Type;
use openrows_driver_api::{BatchPolicy, DriverError};
use std::collections::HashMap;

/// A binary wire array: dimensions as (length, lower bound), then elements.
fn wire_array(element_oid: u32, dimensions: &[(i32, i32)], elements: &[Option<&[u8]>]) -> Vec<u8> {
    let mut out = Vec::new();
    let count = i32::try_from(dimensions.len()).unwrap_or_default();
    out.extend_from_slice(&count.to_be_bytes());
    out.extend_from_slice(&0_i32.to_be_bytes());
    out.extend_from_slice(&element_oid.to_be_bytes());
    for (length, lower) in dimensions {
        out.extend_from_slice(&length.to_be_bytes());
        out.extend_from_slice(&lower.to_be_bytes());
    }
    for element in elements {
        match element {
            Some(bytes) => {
                let length = i32::try_from(bytes.len()).unwrap_or_default();
                out.extend_from_slice(&length.to_be_bytes());
                out.extend_from_slice(bytes);
            }
            None => out.extend_from_slice(&(-1_i32).to_be_bytes()),
        }
    }
    out
}

fn row(cell: Option<&[u8]>) -> Vec<u8> {
    match cell {
        Some(bytes) => {
            let mut out = i32::try_from(bytes.len())
                .unwrap_or_default()
                .to_be_bytes()
                .to_vec();
            out.extend_from_slice(bytes);
            out
        }
        None => (-1_i32).to_be_bytes().to_vec(),
    }
}

fn decode(type_oid: u32, cells: &[Option<Vec<u8>>]) -> Result<RecordBatch, DriverError> {
    let desc = [ColumnDesc {
        name: "a".to_owned(),
        type_oid,
        type_modifier: -1,
    }];
    let mut decoder = ResultDecoder::new(&desc, &HashMap::new(), BatchPolicy::default());
    for cell in cells {
        decoder.decode_row(&row(cell.as_deref()))?;
    }
    decoder
        .finish()?
        .ok_or_else(|| DriverError::Protocol("expected a batch".to_owned()))
}

fn int4(value: i32) -> [u8; 4] {
    value.to_be_bytes()
}

#[test]
fn one_dimensional_arrays_become_lists_with_nulls() -> Result<(), DriverError> {
    let (one, two) = (int4(1), int4(2));
    let cells = [
        Some(wire_array(23, &[(3, 1)], &[Some(&one), None, Some(&two)])),
        None,
        Some(wire_array(23, &[], &[])),
    ];
    let batch = decode(1007, &cells)?;
    let list = batch.column(0).as_list::<i32>();
    assert_eq!(list.len(), 3);
    let first = list.value(0);
    let first = first.as_primitive::<Int32Type>();
    assert_eq!(first.iter().collect::<Vec<_>>(), [Some(1), None, Some(2)]);
    assert!(list.is_null(1));
    assert_eq!(list.value(2).len(), 0);
    Ok(())
}

#[test]
fn other_shapes_switch_the_column_to_array_text() -> Result<(), DriverError> {
    let (one, two, three) = (int4(1), int4(2), int4(3));
    let cells = [
        Some(wire_array(23, &[(1, 1)], &[Some(&one)])),
        Some(wire_array(
            23,
            &[(2, 1), (1, 1)],
            &[Some(&two), Some(&three)],
        )),
        Some(wire_array(23, &[(2, 0)], &[None, Some(&one)])),
    ];
    let batch = decode(1007, &cells)?;
    let schema = batch.schema();
    assert_eq!(schema.field(0).data_type(), &DataType::Utf8);
    assert_eq!(schema.field(0).metadata()["openrows.text_fallback"], "true");
    assert_eq!(
        schema.field(0).metadata()["openrows.source_type"],
        "oid:1007"
    );
    let texts: Vec<_> = batch.column(0).as_string::<i32>().iter().collect();
    assert_eq!(
        texts,
        [Some("{1}"), Some("{{2},{3}}"), Some("[0:1]={NULL,1}")]
    );
    Ok(())
}

#[test]
fn text_items_are_quoted_like_array_out() -> Result<(), CellError> {
    let items: [&[u8]; 5] = [b"plain", b"", b"a b", b"NULL", b"q\"\\"];
    let elements: Vec<Option<&[u8]>> = items.iter().map(|item| Some(*item)).collect();
    let mut out = String::new();
    write_text(
        ColumnKind::Text,
        &wire_array(25, &[(5, 1)], &elements),
        &mut out,
    )?;
    assert_eq!(out, r#"{plain,"","a b","NULL","q\"\\"}"#);
    Ok(())
}

#[test]
fn an_unrepresentable_element_converts_earlier_lists_to_text() -> Result<(), DriverError> {
    let epoch = 0_i64.to_be_bytes();
    let infinity = i64::MAX.to_be_bytes();
    let cells = [
        Some(wire_array(1114, &[(2, 1)], &[Some(&epoch), None])),
        Some(wire_array(1114, &[(1, 1)], &[Some(&infinity)])),
    ];
    let batch = decode(1115, &cells)?;
    let texts: Vec<_> = batch.column(0).as_string::<i32>().iter().collect();
    assert_eq!(
        texts,
        [Some(r#"{"2000-01-01 00:00:00",NULL}"#), Some("{infinity}")]
    );
    Ok(())
}

#[test]
fn truncated_arrays_are_errors() {
    assert!(matches!(header(&[0, 0, 0, 1]), Err(CellError::Truncated)));
    let mut bytes = wire_array(23, &[(1, 1)], &[]);
    bytes.truncate(16);
    assert!(matches!(header(&bytes), Err(CellError::Truncated)));
}
