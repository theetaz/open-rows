// SPDX-License-Identifier: Apache-2.0
//! Golden protocol responses, fragmented bodies, schema reuse and row caps.
use super::*;
use arrow_array::{BinaryArray, Int64Array};
use arrow_schema::DataType;
use std::io::{self, Read};

fn decoded(input: &[u8], cap: usize) -> Result<Vec<Event>, DriverError> {
    let (sender, mut events) = mpsc::channel(2048);
    let limits = ResultLimits {
        rows: cap,
        ..ResultLimits::default()
    };
    parse(Fragmented(input), sender, limits)?;
    let mut found = Vec::new();
    while let Ok(event) = events.try_recv() {
        found.push(event);
    }
    Ok(found)
}
struct Fragmented<'a>(&'a [u8]);
impl Read for Fragmented<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let count = self.0.len().min(buffer.len()).min(3);
        buffer[..count].copy_from_slice(&self.0[..count]);
        self.0 = &self.0[count..];
        Ok(count)
    }
}
#[test]
fn golden_response_preserves_types_and_declared_metadata() -> Result<(), DriverError> {
    let events = decoded(include_bytes!("../../tests/golden/response.json"), 10_000)?;
    let batch = events
        .iter()
        .find_map(|event| {
            if let Event::Batch(batch, _) = event {
                Some(batch)
            } else {
                None
            }
        })
        .ok_or_else(error::protocol)?;
    assert_eq!(batch.schema().field(0).data_type(), &DataType::Int64);
    assert_eq!(
        batch
            .schema()
            .field(0)
            .metadata()
            .get("openrows.source_type")
            .map(String::as_str),
        Some("INTEGER")
    );
    let integer = batch
        .column(0)
        .as_any()
        .downcast_ref::<Int64Array>()
        .ok_or_else(error::protocol)?;
    let blob = batch
        .column(3)
        .as_any()
        .downcast_ref::<BinaryArray>()
        .ok_or_else(error::protocol)?;
    assert_eq!(integer.value(0), i64::MAX);
    assert_eq!(blob.value(0), &[0, 255]);
    Ok(())
}
#[test]
fn row_cap_drains_excess_and_warns_without_large_batches() -> Result<(), DriverError> {
    let mut response: serde_json::Value =
        serde_json::from_slice(include_bytes!("../../tests/golden/response.json"))
            .map_err(|_| error::protocol())?;
    let result = &mut response["results"][0]["response"]["result"];
    let row = result["rows"][0].clone();
    result["rows"] = serde_json::Value::Array(vec![row; 1000]);
    let bytes = serde_json::to_vec(&response).map_err(|_| error::protocol())?;
    let events = decoded(&bytes, 201)?;
    let counts: Vec<_> = events
        .iter()
        .filter_map(|event| {
            if let Event::Batch(batch, _) = event {
                Some(batch.num_rows())
            } else {
                None
            }
        })
        .collect();
    assert_eq!(counts, [200, 1]);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::Truncated(201)))
    );
    Ok(())
}
#[test]
fn rows_before_columns_and_empty_results_are_supported() -> Result<(), DriverError> {
    let input = br#"{"baton":null,"base_url":null,"results":[{"type":"ok","response":{"type":"execute","result":{"rows":[[{"type":"integer","value":"1"}]],"cols":[{"name":"n","decltype":null}],"affected_row_count":0}}}]}"#;
    let events = decoded(input, 5)?;
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::Batch(batch, _) if batch.num_rows() == 1))
    );
    assert!(decoded(br#"{"baton":"test-only","baton":null,"results":[]}"#, 5).is_err());
    Ok(())
}

#[test]
fn unrecognized_nested_rows_never_publish_results() -> Result<(), DriverError> {
    let input = br#"{"baton":null,"results":[],"future":{"cols":[{"name":"x"}],"rows":[[{"type":"integer","value":"1"}]]}}"#;
    assert!(decoded(input, 5)?.is_empty());
    Ok(())
}

#[test]
fn invalid_storage_values_return_value_free_errors() {
    for value in [
        crate::Value::Integer {
            value: "test-only".into(),
        },
        crate::Value::Blob {
            base64: "test-only".into(),
        },
    ] {
        let error = value.into_cell(0).expect_err("invalid value accepted");
        assert!(matches!(error, DriverError::Decode { column: 0, .. }));
        assert!(!format!("{error:?} {error}").contains("test-only"));
    }
}
