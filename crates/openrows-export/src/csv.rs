// SPDX-License-Identifier: Apache-2.0
//! Delimited text (RFC 4180 by default): fields holding the delimiter, the
//! quote, CR or LF are quoted with embedded quotes doubled. NULL is the
//! NULL text (empty by default); an empty string, or a value equal to the
//! NULL text, is quoted so the two stay distinguishable (as in
//! `PostgreSQL`'s `COPY … CSV`). TSV is the same with a tab delimiter.
use crate::cell::{Cell, Column};
use crate::exporter::Encode;
use crate::text::push_plain;
use crate::{CsvOptions, ExportError};
use arrow_schema::Schema;

pub(crate) struct CsvEncoder {
    options: CsvOptions,
    scratch: String,
}

impl CsvEncoder {
    pub(crate) fn new(options: CsvOptions) -> Self {
        Self {
            options,
            scratch: String::new(),
        }
    }

    fn field(&mut self, cell: &Cell<'_>, out: &mut String) {
        if *cell == Cell::Null {
            out.push_str(&self.options.null_text);
            return;
        }
        self.scratch.clear();
        push_plain(cell, &mut self.scratch);
        let forced = self.scratch == self.options.null_text;
        push_field(&self.scratch, &self.options, forced, out);
    }

    fn record(&mut self, columns: &[Column<'_>], row: usize, out: &mut String) {
        let delimiter = self.options.delimiter.as_char();
        for (index, column) in columns.iter().enumerate() {
            if index > 0 {
                out.push(delimiter);
            }
            self.field(&column.cell(row), out);
        }
        out.push_str(self.options.line_ending.as_str());
    }
}

/// Append `value`, quoted when `forced`, empty or holding a special character.
fn push_field(value: &str, options: &CsvOptions, forced: bool, out: &mut String) {
    let delimiter = options.delimiter.as_char();
    let quote = options.quote.as_char();
    let special = |c: char| c == delimiter || c == quote || matches!(c, '\r' | '\n');
    if !forced && !value.is_empty() && !value.contains(special) {
        out.push_str(value);
        return;
    }
    out.push(quote);
    for part in value.split_inclusive(quote) {
        out.push_str(part);
        if part.ends_with(quote) {
            out.push(quote);
        }
    }
    out.push(quote);
}

fn push_header(schema: &Schema, options: &CsvOptions, out: &mut String) {
    let delimiter = options.delimiter.as_char();
    for (index, field) in schema.fields().iter().enumerate() {
        if index > 0 {
            out.push(delimiter);
        }
        push_field(field.name(), options, false, out);
    }
    out.push_str(options.line_ending.as_str());
}

impl Encode for CsvEncoder {
    fn begin(&mut self, schema: &Schema, out: &mut String) -> Result<(), ExportError> {
        if self.options.header {
            push_header(schema, &self.options, out);
        }
        Ok(())
    }

    fn rows(&mut self, columns: &[Column<'_>], rows: usize, out: &mut String) {
        for row in 0..rows {
            self.record(columns, row, out);
        }
    }

    fn finish(&mut self, _out: &mut String) {}
}
