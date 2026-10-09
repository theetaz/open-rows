// SPDX-License-Identifier: Apache-2.0
//! One export: an encoder writing batches into a temporary file.
use crate::cell::Column;
use crate::csv::CsvEncoder;
use crate::encoding::{encode, preamble};
use crate::file::ExportFile;
use crate::json::JsonEncoder;
use crate::markdown::MarkdownEncoder;
use crate::parquet::ParquetWriter;
use crate::sql::InsertEncoder;
use crate::xlsx::ExcelWriter;
use crate::{ExportError, Format, TextEncoding};
use arrow_array::{RecordBatch, new_empty_array};
use arrow_schema::{Schema, SchemaRef};
use std::path::Path;

/// A text format encoder; called once per batch, never per row.
pub(crate) trait Encode: Send {
    /// Validate the columns and write any preamble (CSV header, `[`).
    fn begin(&mut self, schema: &Schema, out: &mut String) -> Result<(), ExportError>;
    /// Append `rows` rows of `columns`.
    fn rows(&mut self, columns: &[Column<'_>], rows: usize, out: &mut String);
    /// Close any open statement or array.
    fn finish(&mut self, out: &mut String);
}

/// A binary format writer with its own handle on the temporary file.
pub(crate) trait WriteBatches: Send {
    /// A result with these columns starts.
    fn begin(&mut self, schema: &SchemaRef) -> Result<(), ExportError>;
    /// The driver continued the result with new column types.
    fn resume(&mut self, schema: &SchemaRef) -> Result<(), ExportError>;
    /// The next result is another table named `name`.
    fn table(&mut self, name: &str) -> Result<(), ExportError>;
    /// Write one batch, also given as typed columns.
    fn write(&mut self, columns: &[Column<'_>], batch: &RecordBatch) -> Result<(), ExportError>;
    /// Write everything still buffered to the file.
    fn finish(&mut self) -> Result<(), ExportError>;
}

/// Text formats encode into a reused buffer that is transcoded and written.
struct TextOutput {
    encoder: Box<dyn Encode>,
    encoding: TextEncoding,
    buffer: String,
    bytes: Vec<u8>,
}

impl Output {
    fn text(encoder: Box<dyn Encode>, encoding: TextEncoding) -> Self {
        Self::Text(TextOutput {
            encoder,
            encoding,
            buffer: String::new(),
            bytes: Vec::new(),
        })
    }
}

enum Output {
    Text(TextOutput),
    Batches(Box<dyn WriteBatches>),
}

/// Encodes one result (or, for Excel, several tables) into a file; see the
/// crate docs for the file lifecycle.
pub struct Exporter {
    file: ExportFile,
    output: Output,
    schema: Option<SchemaRef>,
    rows: u64,
}

impl Exporter {
    /// Create the temporary file next to `path` for `format`.
    ///
    /// # Errors
    /// [`ExportError::InvalidPath`] or the I/O error creating the file.
    pub fn create(path: &Path, format: &Format) -> Result<Self, ExportError> {
        let mut file = ExportFile::create(path)?;
        let output = match format {
            Format::Csv(options) => {
                Output::text(Box::new(CsvEncoder::new(options.clone())), options.encoding)
            }
            Format::Json(layout) => {
                Output::text(Box::new(JsonEncoder::new(*layout)), TextEncoding::Utf8)
            }
            Format::Insert(options) => Output::text(
                Box::new(InsertEncoder::new(options.clone())),
                TextEncoding::Utf8,
            ),
            Format::Markdown => {
                Output::text(Box::new(MarkdownEncoder::default()), TextEncoding::Utf8)
            }
            Format::Excel(options) => {
                Output::Batches(Box::new(ExcelWriter::new(file.handle()?, options.clone())))
            }
            Format::Parquet(compression) => {
                Output::Batches(Box::new(ParquetWriter::new(file.handle()?, *compression)))
            }
        };
        if let Output::Text(text) = &output {
            file.write(preamble(text.encoding))?;
        }
        Ok(Self {
            file,
            output,
            schema: None,
            rows: 0,
        })
    }

    #[cfg(test)]
    pub(crate) fn excel_with_sheet_rows(
        path: &Path,
        options: crate::ExcelOptions,
        rows: u32,
    ) -> Result<Self, ExportError> {
        let mut file = ExportFile::create(path)?;
        let writer = ExcelWriter::new(file.handle()?, options).with_sheet_rows(rows);
        Ok(Self {
            file,
            output: Output::Batches(Box::new(writer)),
            schema: None,
            rows: 0,
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
            if let Output::Batches(writer) = &mut self.output {
                writer.resume(&schema)?;
            }
            self.schema = Some(schema);
            return Ok(());
        }
        match &mut self.output {
            Output::Text(text) => text.encoder.begin(&schema, &mut text.buffer)?,
            Output::Batches(writer) => writer.begin(&schema)?,
        }
        self.schema = Some(schema);
        self.flush()
    }

    /// Start the next table of a multi-table file (an Excel sheet per
    /// table); [`Exporter::begin`] follows with its columns.
    ///
    /// # Errors
    /// [`ExportError::OneTablePerFile`] for formats that hold one result.
    pub fn start_table(&mut self, name: &str) -> Result<(), ExportError> {
        match &mut self.output {
            Output::Batches(writer) => writer.table(name)?,
            Output::Text(_) => return Err(ExportError::OneTablePerFile),
        }
        self.schema = None;
        Ok(())
    }

    /// Encode and write one batch.
    ///
    /// # Errors
    /// [`ExportError::SchemaMismatch`] before `begin` or for a different
    /// column count; encoding and I/O errors.
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
        match &mut self.output {
            Output::Text(text) => text
                .encoder
                .rows(&columns, batch.num_rows(), &mut text.buffer),
            Output::Batches(writer) => writer.write(&columns, batch)?,
        }
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
        match &mut self.output {
            Output::Text(text) => text.encoder.finish(&mut text.buffer),
            Output::Batches(writer) => writer.finish()?,
        }
        self.flush()?;
        self.file.commit()?;
        Ok(self.rows)
    }

    fn flush(&mut self) -> Result<(), ExportError> {
        let Output::Text(text) = &mut self.output else {
            return Ok(());
        };
        if text.buffer.is_empty() {
            return Ok(());
        }
        let bytes = encode(&text.buffer, text.encoding, &mut text.bytes)?;
        let written = self.file.write(bytes);
        text.buffer.clear();
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
