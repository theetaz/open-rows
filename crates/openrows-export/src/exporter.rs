// SPDX-License-Identifier: Apache-2.0
//! One export: an encoder writing batches into a temporary file.
use crate::cell::Column;
use crate::csv::CsvEncoder;
use crate::file::ExportFile;
use crate::json::JsonEncoder;
use crate::sql::InsertEncoder;
use crate::{ExportError, Format};
use arrow_array::{RecordBatch, new_empty_array};
use arrow_schema::{Schema, SchemaRef};
use std::path::Path;

/// A format encoder; called once per batch, never per row.
pub(crate) trait Encode: Send {
    /// Validate the columns and write any preamble (CSV header, `[`).
    fn begin(&mut self, schema: &Schema, out: &mut String) -> Result<(), ExportError>;
    /// Append `rows` rows of `columns`.
    fn rows(&mut self, columns: &[Column<'_>], rows: usize, out: &mut String);
    /// Close any open statement or array.
    fn finish(&mut self, out: &mut String);
}

/// Encodes one result into a file; see the crate docs for the file lifecycle.
pub struct Exporter {
    file: ExportFile,
    encoder: Box<dyn Encode>,
    schema: Option<SchemaRef>,
    rows: u64,
    /// Encoded text of the current batch, reused across batches.
    buffer: String,
}

impl Exporter {
    /// Create the temporary file next to `path` for `format`.
    ///
    /// # Errors
    /// [`ExportError::InvalidPath`] or the I/O error creating the file.
    pub fn create(path: &Path, format: &Format) -> Result<Self, ExportError> {
        let encoder: Box<dyn Encode> = match format {
            Format::Csv(options) => Box::new(CsvEncoder::new(*options)),
            Format::Json(layout) => Box::new(JsonEncoder::new(*layout)),
            Format::Insert(options) => Box::new(InsertEncoder::new(options.clone())),
        };
        Ok(Self {
            file: ExportFile::create(path)?,
            encoder,
            schema: None,
            rows: 0,
            buffer: String::new(),
        })
    }

    /// Rows written so far.
    #[must_use]
    pub fn rows(&self) -> u64 {
        self.rows
    }

    /// Start the result: check every column type and write the preamble.
    ///
    /// A later call continues the same output with new column types, as
    /// when a driver restarts a result because a column fell back to text
    /// (docs/05 §11); the column names must stay the same.
    ///
    /// # Errors
    /// Unsupported column types, invalid SQL names, a continuation with
    /// other columns ([`ExportError::SchemaMismatch`]) or I/O errors.
    pub fn begin(&mut self, schema: SchemaRef) -> Result<(), ExportError> {
        for field in schema.fields() {
            Column::new(field, new_empty_array(field.data_type()).as_ref())?;
        }
        if let Some(current) = &self.schema {
            let names = |schema: &Schema| -> Vec<String> {
                schema.fields().iter().map(|f| f.name().clone()).collect()
            };
            if names(current) != names(&schema) {
                return Err(ExportError::SchemaMismatch);
            }
            self.schema = Some(schema);
            return Ok(());
        }
        self.encoder.begin(&schema, &mut self.buffer)?;
        self.schema = Some(schema);
        self.flush()
    }

    /// Encode and write one batch.
    ///
    /// # Errors
    /// [`ExportError::SchemaMismatch`] before `begin` or for a different
    /// column count; I/O errors.
    pub fn write(&mut self, batch: &RecordBatch) -> Result<(), ExportError> {
        let schema = self.schema.as_ref().ok_or(ExportError::SchemaMismatch)?;
        if batch.num_columns() != schema.fields().len() {
            return Err(ExportError::SchemaMismatch);
        }
        let columns = schema
            .fields()
            .iter()
            .zip(batch.columns())
            .map(|(field, array)| Column::new(field, array.as_ref()))
            .collect::<Result<Vec<_>, _>>()?;
        self.encoder
            .rows(&columns, batch.num_rows(), &mut self.buffer);
        self.rows += batch.num_rows() as u64;
        self.flush()
    }

    /// Close the output, sync it and rename it into place; returns the row count.
    ///
    /// # Errors
    /// [`ExportError::NoResult`] when no result began; I/O errors. The
    /// temporary file is deleted on any error.
    pub fn finish(mut self) -> Result<u64, ExportError> {
        if self.schema.is_none() {
            return Err(ExportError::NoResult);
        }
        self.encoder.finish(&mut self.buffer);
        self.flush()?;
        self.file.commit()?;
        Ok(self.rows)
    }

    fn flush(&mut self) -> Result<(), ExportError> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let written = self.file.write(self.buffer.as_bytes());
        self.buffer.clear();
        written
    }
}

impl std::fmt::Debug for Exporter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Exporter")
            .field("rows", &self.rows)
            .field("begun", &self.schema.is_some())
            .finish_non_exhaustive()
    }
}
