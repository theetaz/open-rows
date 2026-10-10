// SPDX-License-Identifier: Apache-2.0
//! Preserve opaque scalar values inside native nested Arrow arrays before display.
use duckdb::arrow::{
    array::{ArrayRef, make_array},
    datatypes::{DataType, Field, Fields, UnionFields},
};
use openrows_driver_api::{
    DriverError,
    metadata::{DECIMAL_EXTENSION, EXTENSION_NAME, SOURCE_TYPE, TEXT_FALLBACK},
};
use std::sync::Arc;

pub(super) fn normalize(
    field: &Field,
    array: &ArrayRef,
    depth: u8,
) -> Result<(Field, ArrayRef), DriverError> {
    if depth > 16 {
        return Err(DriverError::Unsupported(
            "DuckDB nested type exceeds 16 levels".into(),
        ));
    }
    let opaque = field
        .metadata()
        .get("ARROW:extension:metadata")
        .map(|v| v.to_ascii_lowercase())
        .unwrap_or_default();
    let output = scalar(field, array, &opaque, depth)?;
    Ok((canonical_field(field, &output, &opaque), output))
}
fn canonical_field(field: &Field, output: &ArrayRef, opaque: &str) -> Field {
    let mut metadata = field.metadata().clone();
    if output.data_type() != field.data_type() {
        metadata.remove(EXTENSION_NAME);
        metadata.remove("ARROW:extension:metadata");
        if opaque.contains("hugeint") {
            metadata.insert(EXTENSION_NAME.into(), DECIMAL_EXTENSION.into());
            metadata.insert(TEXT_FALLBACK.into(), "true".into());
            metadata.insert(
                SOURCE_TYPE.into(),
                if opaque.contains("uhugeint") {
                    "UHUGEINT"
                } else {
                    "HUGEINT"
                }
                .into(),
            );
        }
    }
    Field::new(
        field.name(),
        output.data_type().clone(),
        field.is_nullable(),
    )
    .with_metadata(metadata)
}
fn scalar(
    field: &Field,
    array: &ArrayRef,
    opaque: &str,
    depth: u8,
) -> Result<ArrayRef, DriverError> {
    let output = if opaque.contains("uhugeint") {
        super::bridge::hugeints(array.as_ref(), true)?
    } else if opaque.contains("hugeint") {
        super::bridge::hugeints(array.as_ref(), false)?
    } else if (field
        .metadata()
        .get(EXTENSION_NAME)
        .is_some_and(|name| name == "arrow.bool8")
        || opaque.contains("bool"))
        && array.data_type() == &DataType::Int8
    {
        let values = array
            .as_any()
            .downcast_ref::<duckdb::arrow::array::Int8Array>()
            .ok_or_else(|| {
                DriverError::Protocol("DuckDB nested boolean storage mismatch".into())
            })?;
        Arc::new(
            values
                .iter()
                .map(|v| v.map(|v| v != 0))
                .collect::<duckdb::arrow::array::BooleanArray>(),
        ) as ArrayRef
    } else if matches!(array.data_type(), DataType::Timestamp(duckdb::arrow::datatypes::TimeUnit::Microsecond, Some(zone)) if zone.as_ref() == "UTC")
    {
        let values = array
            .as_any()
            .downcast_ref::<duckdb::arrow::array::TimestampMicrosecondArray>()
            .ok_or_else(|| DriverError::Protocol("DuckDB timestamp storage mismatch".into()))?;
        Arc::new(values.clone().with_timezone("+00:00")) as ArrayRef
    } else if matches!(array.data_type(), DataType::Dictionary(..)) {
        duckdb::arrow::compute::cast(array.as_ref(), &DataType::Utf8)
            .map_err(|e| super::bridge::arrow_error(&e))?
    } else {
        children(field, array, depth)?
    };
    Ok(output)
}

fn children(field: &Field, array: &ArrayRef, depth: u8) -> Result<ArrayRef, DriverError> {
    let fields: Vec<_> = match field.data_type() {
        DataType::List(child)
        | DataType::LargeList(child)
        | DataType::FixedSizeList(child, _)
        | DataType::Map(child, _) => vec![child.clone()],
        DataType::Struct(fields) => fields.iter().cloned().collect(),
        DataType::Union(fields, _) => fields.iter().map(|(_, field)| field.clone()).collect(),
        _ => return Ok(array.clone()),
    };
    let data = array.to_data();
    let normalized = fields
        .iter()
        .zip(data.child_data())
        .map(|(field, data)| normalize(field, &make_array(data.clone()), depth + 1))
        .collect::<Result<Vec<_>, _>>()?;
    let fields: Fields = normalized
        .iter()
        .map(|(field, _)| Arc::new(field.clone()))
        .collect();
    let datatype = match field.data_type() {
        DataType::List(_) => DataType::List(fields[0].clone()),
        DataType::LargeList(_) => DataType::LargeList(fields[0].clone()),
        DataType::FixedSizeList(_, width) => DataType::FixedSizeList(fields[0].clone(), *width),
        DataType::Map(_, ordered) => DataType::Map(fields[0].clone(), *ordered),
        DataType::Struct(_) => DataType::Struct(fields),
        DataType::Union(original, mode) => DataType::Union(
            UnionFields::try_new(original.iter().map(|(id, _)| id), fields.iter().cloned())
                .map_err(|e| super::bridge::arrow_error(&e))?,
            *mode,
        ),
        _ => return Err(DriverError::Protocol("DuckDB nested schema changed".into())),
    };
    let data = data
        .into_builder()
        .data_type(datatype)
        .child_data(
            normalized
                .iter()
                .map(|(_, array)| array.to_data())
                .collect(),
        )
        .build()
        .map_err(|e| super::bridge::arrow_error(&e))?;
    Ok(make_array(data))
}
