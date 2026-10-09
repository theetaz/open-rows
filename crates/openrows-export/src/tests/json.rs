// SPDX-License-Identifier: Apache-2.0
use super::{TestResult, export, export_one, mixed, texts};
use crate::{Format, JsonLayout};
use arrow_array::{ArrayRef, Float32Array, Float64Array, RecordBatch};
use arrow_schema::{DataType, Field, Schema};
use std::sync::Arc;

#[test]
fn rows_become_typed_objects_in_an_array() -> TestResult {
    let text = export_one(&Format::Json(JsonLayout::Array), &mixed()?)?;
    assert_eq!(
        text,
        "[\n{\"id\":-7,\"big\":18446744073709551615,\"ok\":true,\"ratio\":0.5,\"price\":-12.34,\
         \"name\":\"it's \\\"ü\\\", ok\",\"doc\":{\"a\": [1, 2]},\"raw\":\"\\\\x00ff10\",\
         \"uid\":\"550e8400-e29b-41d4-a716-446655440000\",\"day\":\"2024-02-29\",\
         \"at\":\"2024-02-29 12:34:56.123456+00:00\"},\n\
         {\"id\":null,\"big\":null,\"ok\":null,\"ratio\":null,\"price\":null,\"name\":null,\
         \"doc\":null,\"raw\":null,\"uid\":null,\"day\":null,\"at\":null}\n]\n"
    );
    Ok(())
}

#[test]
fn ndjson_writes_one_object_per_line() -> TestResult {
    let batch = texts(&[Some("a"), None])?;
    let text = export_one(&Format::Json(JsonLayout::Lines), &batch)?;
    assert_eq!(text, "{\"v\":\"a\"}\n{\"v\":null}\n");
    let empty = export(&Format::Json(JsonLayout::Lines), batch.schema(), &[])?;
    assert_eq!(empty, "");
    Ok(())
}

#[test]
fn an_empty_result_is_an_empty_array() -> TestResult {
    let batch = texts(&[])?;
    assert_eq!(
        export_one(&Format::Json(JsonLayout::Array), &batch)?,
        "[]\n"
    );
    Ok(())
}

#[test]
fn control_characters_and_quotes_are_escaped() -> TestResult {
    let batch = texts(&[Some("a\"b\\c\n\r\t\u{08}\u{0c}\u{01}\u{1f}é😀")])?;
    let text = export_one(&Format::Json(JsonLayout::Lines), &batch)?;
    assert_eq!(
        text,
        "{\"v\":\"a\\\"b\\\\c\\n\\r\\t\\b\\f\\u0001\\u001fé😀\"}\n"
    );
    Ok(())
}

#[test]
fn non_finite_floats_become_strings_and_finite_ones_round_trip() -> TestResult {
    let schema = Arc::new(Schema::new(vec![
        Field::new("d", DataType::Float64, true),
        Field::new("f", DataType::Float32, true),
    ]));
    let columns: Vec<ArrayRef> = vec![
        Arc::new(Float64Array::from(vec![f64::NAN, f64::INFINITY, 0.1, 1e-7])),
        Arc::new(Float32Array::from(vec![
            f32::NEG_INFINITY,
            0.1,
            -0.0,
            3.5e10,
        ])),
    ];
    let batch = RecordBatch::try_new(schema, columns)?;
    let text = export_one(&Format::Json(JsonLayout::Lines), &batch)?;
    assert_eq!(
        text,
        "{\"d\":\"NaN\",\"f\":\"-Infinity\"}\n{\"d\":\"Infinity\",\"f\":0.1}\n\
         {\"d\":0.1,\"f\":-0}\n{\"d\":1e-7,\"f\":35000000000}\n"
    );
    Ok(())
}

#[test]
fn pretty_json_indents_objects_by_two_spaces() -> TestResult {
    let batch = texts(&[Some("a"), None])?;
    let text = export_one(&Format::Json(JsonLayout::Pretty), &batch)?;
    assert_eq!(
        text,
        "[\n  {\n    \"v\": \"a\"\n  },\n  {\n    \"v\": null\n  }\n]\n"
    );
    Ok(())
}
