// SPDX-License-Identifier: Apache-2.0
//! Apache Parquet through the `parquet` crate's Arrow writer: columns keep
//! their Arrow types, except intervals, which Parquet cannot store with
//! nanoseconds and are written as their canonical text. Row groups are
//! flushed every [`ROW_GROUP_ROWS`] rows or [`ROW_GROUP_BYTES`] bytes, so
//! memory stays bounded however long the result is.
use crate::cell::Column;
use crate::exporter::WriteBatches;
use crate::text::push_plain;
use crate::{ExportError, ParquetCompression};
use arrow_array::{ArrayRef, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema, SchemaRef};
use parquet::arrow::ArrowWriter;
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;
use std::fs::File;
use std::sync::Arc;

/// Rows per row group.
const ROW_GROUP_ROWS: usize = 128 * 1024;
/// Buffered bytes that close a row group early (wide rows).
const ROW_GROUP_BYTES: usize = 128 * 1024 * 1024;

pub(crate) struct ParquetWriter {
    file: Option<File>,
    compression: ParquetCompression,
    writer: Option<ArrowWriter<File>>,
    /// The file's schema and, per column, whether it is written as text.
    schema: Option<(SchemaRef, Vec<bool>)>,
}

fn parquet(error: impl std::fmt::Display) -> ExportError {
    ExportError::Parquet(error.to_string())
}

impl ParquetWriter {
    pub(crate) fn new(file: File, compression: ParquetCompression) -> Self {
        Self {
            file: Some(file),
            compression,
            writer: None,
            schema: None,
        }
    }

    fn properties(&self) -> WriterProperties {
        let compression = match self.compression {
            ParquetCompression::Snappy => Compression::SNAPPY,
            ParquetCompression::Uncompressed => Compression::UNCOMPRESSED,
        };
        WriterProperties::builder()
            .set_compression(compression)
            .set_max_row_group_row_count(Some(ROW_GROUP_ROWS))
            .set_created_by(format!("OpenRows {}", env!("CARGO_PKG_VERSION")))
            .build()
    }
}

/// The file schema for `schema`: intervals become text columns.
fn file_schema(schema: &Schema) -> (SchemaRef, Vec<bool>) {
    let text: Vec<bool> = schema
        .fields()
        .iter()
        .map(|field| matches!(field.data_type(), DataType::Interval(_)))
        .collect();
    let fields: Vec<Field> = schema
        .fields()
        .iter()
        .zip(&text)
        .map(|(field, text)| {
            if *text {
                Field::new(field.name(), DataType::Utf8, true)
            } else {
                field.as_ref().clone()
            }
        })
        .collect();
    (Arc::new(Schema::new(fields)), text)
}

/// The canonical text of every value of `column`.
fn as_text(column: &Column<'_>, rows: usize) -> ArrayRef {
    let mut scratch = String::new();
    let values: StringArray = (0..rows)
        .map(|row| {
            let cell = column.cell(row);
            (cell != crate::cell::Cell::Null).then(|| {
                scratch.clear();
                push_plain(&cell, &mut scratch);
                scratch.clone()
            })
        })
        .collect();
    Arc::new(values)
}

impl WriteBatches for ParquetWriter {
    fn begin(&mut self, schema: &SchemaRef) -> Result<(), ExportError> {
        let file = self.file.take().ok_or(ExportError::OneTablePerFile)?;
        let (file_schema, text) = file_schema(schema);
        let writer = ArrowWriter::try_new(file, Arc::clone(&file_schema), Some(self.properties()))
            .map_err(parquet)?;
        self.writer = Some(writer);
        self.schema = Some((file_schema, text));
        Ok(())
    }

    /// A Parquet file has one schema: a continuation must keep the types.
    fn resume(&mut self, schema: &SchemaRef) -> Result<(), ExportError> {
        let (current, _) = self.schema.as_ref().ok_or(ExportError::SchemaMismatch)?;
        let (next, _) = file_schema(schema);
        let types = |schema: &Schema| -> Vec<DataType> {
            schema
                .fields()
                .iter()
                .map(|f| f.data_type().clone())
                .collect()
        };
        if types(current) == types(&next) {
            Ok(())
        } else {
            Err(ExportError::Parquet(
                "the server changed a column's type during the result; export as CSV instead"
                    .to_owned(),
            ))
        }
    }

    fn table(&mut self, _name: &str) -> Result<(), ExportError> {
        Err(ExportError::OneTablePerFile)
    }

    fn write(&mut self, columns: &[Column<'_>], batch: &RecordBatch) -> Result<(), ExportError> {
        let (schema, text) = self.schema.as_ref().ok_or(ExportError::SchemaMismatch)?;
        let writer = self.writer.as_mut().ok_or(ExportError::SchemaMismatch)?;
        let arrays: Vec<ArrayRef> = batch
            .columns()
            .iter()
            .zip(columns)
            .zip(text)
            .map(|((array, column), text)| {
                if *text {
                    as_text(column, batch.num_rows())
                } else {
                    Arc::clone(array)
                }
            })
            .collect();
        let converted = RecordBatch::try_new(Arc::clone(schema), arrays).map_err(parquet)?;
        writer.write(&converted).map_err(parquet)?;
        if writer.in_progress_size() > ROW_GROUP_BYTES {
            writer.flush().map_err(parquet)?;
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), ExportError> {
        let writer = self.writer.take().ok_or(ExportError::NoResult)?;
        writer.close().map_err(parquet).map(drop)
    }
}
