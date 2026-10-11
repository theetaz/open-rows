// SPDX-License-Identifier: Apache-2.0
//! Bounded native Arrow 58 → workspace Arrow 60 IPC bridge, without per-cell JSON.
use duckdb::arrow::{
    array::{Array, ArrayRef, FixedSizeBinaryArray, RecordBatch, StringArray},
    datatypes::{DataType, Field, Schema},
};
use openrows_driver_api::{
    DriverError,
    metadata::{DECIMAL_EXTENSION, EXTENSION_NAME, SOURCE_TYPE, TEXT_FALLBACK},
};
use std::{
    io::{Cursor, Write},
    sync::Arc,
};

#[derive(Default)]
struct Capped(Vec<u8>);
impl Write for Capped {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > openrows_host_protocol::LARGE_LIMIT.saturating_sub(self.0.len()) {
            return Err(std::io::ErrorKind::OutOfMemory.into());
        }
        self.0
            .try_reserve(bytes.len())
            .map_err(|_| std::io::ErrorKind::OutOfMemory)?;
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
pub(super) fn convert(
    batch: &RecordBatch,
    types: &[String],
) -> Result<arrow_array::RecordBatch, DriverError> {
    if batch.get_array_memory_size() > openrows_host_protocol::LARGE_LIMIT {
        return Err(DriverError::Unsupported(
            "DuckDB row exceeds the 64 MiB Arrow ceiling".into(),
        ));
    }
    let normalized = normalize(batch, types)?;
    let mut writer =
        arrow_ipc_58::writer::StreamWriter::try_new(Capped::default(), &normalized.schema())
            .map_err(|e| arrow_error(&e))?;
    writer.write(&normalized).map_err(|e| arrow_error(&e))?;
    writer.finish().map_err(|e| arrow_error(&e))?;
    let bytes = writer.into_inner().map_err(|e| arrow_error(&e))?.0;
    let mut reader = arrow_ipc::reader::StreamReader::try_new(Cursor::new(bytes), None)?;
    reader
        .next()
        .ok_or_else(|| DriverError::Protocol("DuckDB Arrow bridge returned no batch".into()))?
        .map_err(DriverError::from)
}
fn normalize(batch: &RecordBatch, types: &[String]) -> Result<RecordBatch, DriverError> {
    let mut fields = Vec::with_capacity(batch.num_columns());
    let mut columns = Vec::with_capacity(batch.num_columns());
    for (index, (field, column)) in batch
        .schema()
        .fields()
        .iter()
        .zip(batch.columns())
        .enumerate()
    {
        let source = types
            .get(index)
            .ok_or_else(|| DriverError::Protocol("DuckDB column metadata mismatch".into()))?;
        let (field, column) = column_value(field, column, source)?;
        fields.push(field);
        columns.push(column);
    }
    RecordBatch::try_new(Arc::new(Schema::new(fields)), columns).map_err(|e| arrow_error(&e))
}
fn column_value(
    field: &Field,
    array: &ArrayRef,
    source: &str,
) -> Result<(Field, ArrayRef), DriverError> {
    let (normalized_field, normalized) = super::nested::normalize(field, array, 0)?;
    let array = &normalized;
    let mut metadata = normalized_field.metadata().clone();
    metadata.insert(SOURCE_TYPE.into(), source.into());
    let huge = matches!(source, "Hugeint" | "UHugeint");
    let fallback = huge || !shared(field.data_type());
    let output = if huge && array.data_type() != &DataType::Utf8 {
        hugeints(array.as_ref(), source == "UHugeint")?
    } else if fallback {
        text(array.as_ref())?
    } else {
        array.clone()
    };
    if fallback {
        metadata.remove(EXTENSION_NAME);
        metadata.remove("ARROW:extension:metadata");
        metadata.insert(TEXT_FALLBACK.into(), "true".into());
        if huge {
            metadata.insert(EXTENSION_NAME.into(), DECIMAL_EXTENSION.into());
        }
    }
    Ok((
        Field::new(
            field.name(),
            output.data_type().clone(),
            field.is_nullable(),
        )
        .with_metadata(metadata),
        output,
    ))
}
fn shared(data_type: &DataType) -> bool {
    match data_type {
        DataType::Dictionary(..)
        | DataType::Struct(_)
        | DataType::Map(..)
        | DataType::Union(..)
        | DataType::FixedSizeList(..) => false,
        DataType::List(field) | DataType::LargeList(field) => shared(field.data_type()),
        _ => true,
    }
}
pub(super) fn hugeints(array: &dyn Array, unsigned: bool) -> Result<ArrayRef, DriverError> {
    let values = array
        .as_any()
        .downcast_ref::<FixedSizeBinaryArray>()
        .ok_or_else(|| DriverError::Decode {
            column: 0,
            reason: "DuckDB HUGEINT must have its lossless 16-byte Arrow representation".into(),
        })?;
    let rows = (0..array.len())
        .map(|row| {
            if array.is_null(row) {
                return Ok(None);
            }
            let bytes: [u8; 16] =
                values
                    .value(row)
                    .try_into()
                    .map_err(|_| DriverError::Decode {
                        column: 0,
                        reason: "invalid HUGEINT width".into(),
                    })?;
            Ok(Some(if unsigned {
                u128::from_le_bytes(bytes).to_string()
            } else {
                i128::from_le_bytes(bytes).to_string()
            }))
        })
        .collect::<Result<Vec<_>, DriverError>>()?;
    Ok(Arc::new(StringArray::from(rows)))
}
fn text(array: &dyn Array) -> Result<ArrayRef, DriverError> {
    if matches!(array.data_type(), DataType::Dictionary(..)) {
        return duckdb::arrow::compute::cast(array, &DataType::Utf8).map_err(|e| arrow_error(&e));
    }
    let options = duckdb::arrow::util::display::FormatOptions::default().with_null("NULL");
    let formatter = duckdb::arrow::util::display::ArrayFormatter::try_new(array, &options)
        .map_err(|e| arrow_error(&e))?;
    let values = (0..array.len())
        .map(|row| (!array.is_null(row)).then(|| formatter.value(row).to_string()))
        .collect::<Vec<_>>();
    Ok(Arc::new(StringArray::from(values)))
}
pub(super) fn arrow_error(error: &duckdb::arrow::error::ArrowError) -> DriverError {
    DriverError::Unsupported(format!(
        "DuckDB Arrow conversion failed or exceeds the 64 MiB row ceiling: {error}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn oversized_native_array_fails_before_ipc_allocation() -> Result<(), Box<dyn std::error::Error>>
    {
        let text = "x".repeat(openrows_host_protocol::LARGE_LIMIT + 1);
        let batch = RecordBatch::try_from_iter([(
            "value",
            Arc::new(StringArray::from(vec![text])) as ArrayRef,
        )])?;
        assert!(
            matches!(convert(&batch, &["Varchar".into()]), Err(DriverError::Unsupported(message)) if message.contains("64 MiB"))
        );
        Ok(())
    }
}
