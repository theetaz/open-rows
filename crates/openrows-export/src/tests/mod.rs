// SPDX-License-Identifier: Apache-2.0
//! Encoder, literal and file-lifecycle tests on hand-built Arrow batches.
mod csv;
mod files;
mod json;
mod sql;
mod text;

use crate::{ExportError, Exporter, Format};
use arrow_array::{
    ArrayRef, BinaryArray, BooleanArray, Date32Array, Decimal128Array, FixedSizeBinaryArray,
    Float64Array, Int64Array, RecordBatch, StringArray, TimestampMicrosecondArray, UInt64Array,
};
use arrow_schema::{DataType, Field, Schema, SchemaRef, TimeUnit};
use openrows_driver_api::metadata::{EXTENSION_NAME, JSON_EXTENSION, UUID_EXTENSION};
use std::collections::HashMap;
use std::sync::Arc;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn tagged(name: &str, data_type: DataType, extension: &str) -> Field {
    Field::new(name, data_type, true).with_metadata(HashMap::from([(
        EXTENSION_NAME.to_owned(),
        extension.to_owned(),
    )]))
}

/// One column per supported shape, two rows: values, then NULLs.
fn mixed() -> TestResult<RecordBatch> {
    let uuid = [
        0x55, 0x0e, 0x84, 0x00, 0xe2, 0x9b, 0x41, 0xd4, 0xa7, 0x16, 0x44, 0x66, 0x55, 0x44, 0x00,
        0x00,
    ];
    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::Int64, true),
        Field::new("big", DataType::UInt64, true),
        Field::new("ok", DataType::Boolean, true),
        Field::new("ratio", DataType::Float64, true),
        Field::new("price", DataType::Decimal128(10, 2), true),
        Field::new("name", DataType::Utf8, true),
        tagged("doc", DataType::Utf8, JSON_EXTENSION),
        Field::new("raw", DataType::Binary, true),
        tagged("uid", DataType::FixedSizeBinary(16), UUID_EXTENSION),
        Field::new("day", DataType::Date32, true),
        Field::new(
            "at",
            DataType::Timestamp(TimeUnit::Microsecond, Some("UTC".into())),
            true,
        ),
    ]));
    let columns: Vec<ArrayRef> = vec![
        Arc::new(Int64Array::from(vec![Some(-7), None])),
        Arc::new(UInt64Array::from(vec![Some(u64::MAX), None])),
        Arc::new(BooleanArray::from(vec![Some(true), None])),
        Arc::new(Float64Array::from(vec![Some(0.5), None])),
        Arc::new(Decimal128Array::from(vec![Some(-1234), None]).with_precision_and_scale(10, 2)?),
        Arc::new(StringArray::from(vec![Some("it's \"ü\", ok"), None])),
        Arc::new(StringArray::from(vec![Some("{\"a\": [1, 2]}"), None])),
        Arc::new(BinaryArray::from(vec![Some(&[0x00, 0xff, 0x10][..]), None])),
        Arc::new(FixedSizeBinaryArray::try_from_sparse_iter_with_size(
            vec![Some(uuid), None].into_iter(),
            16,
        )?),
        Arc::new(Date32Array::from(vec![Some(19_782), None])),
        Arc::new(
            TimestampMicrosecondArray::from(vec![Some(1_709_210_096_123_456), None])
                .with_timezone("UTC"),
        ),
    ];
    Ok(RecordBatch::try_new(schema, columns)?)
}

/// A one-column text batch.
fn texts(values: &[Option<&str>]) -> TestResult<RecordBatch> {
    let schema = Arc::new(Schema::new(vec![Field::new("v", DataType::Utf8, true)]));
    let column: ArrayRef = Arc::new(StringArray::from(values.to_vec()));
    Ok(RecordBatch::try_new(schema, vec![column])?)
}

/// Export `batches` (sharing `schema`) to a fresh file and read it back.
fn export(format: &Format, schema: SchemaRef, batches: &[RecordBatch]) -> TestResult<String> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("out.txt");
    let rows = crate::write_all(
        Exporter::create(&path, format)?,
        (schema, batches),
        || false,
        |_| {},
    )?;
    let expected: usize = batches.iter().map(RecordBatch::num_rows).sum();
    assert_eq!(rows, expected as u64);
    Ok(std::fs::read_to_string(path)?)
}

fn export_one(format: &Format, batch: &RecordBatch) -> TestResult<String> {
    export(format, batch.schema(), std::slice::from_ref(batch))
}

fn is_unsupported(error: &ExportError) -> bool {
    matches!(error, ExportError::UnsupportedType { .. })
}
