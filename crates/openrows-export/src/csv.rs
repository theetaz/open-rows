// SPDX-License-Identifier: Apache-2.0
//! RFC 4180 CSV: CRLF record ends; fields holding the delimiter, `"`, CR or
//! LF are quoted with embedded quotes doubled. NULL is an empty field and an
//! empty string is `""`, so the two stay distinguishable (as in
//! `PostgreSQL`'s `COPY … CSV`).
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
            return;
        }
        self.scratch.clear();
        push_plain(cell, &mut self.scratch);
        push_field(&self.scratch, self.options.delimiter.as_char(), out);
    }
}

/// Append `value`, quoted when it is empty or holds a special character.
fn push_field(value: &str, delimiter: char, out: &mut String) {
    let special = |c: char| c == delimiter || matches!(c, '"' | '\r' | '\n');
    if !value.is_empty() && !value.contains(special) {
        out.push_str(value);
        return;
    }
    out.push('"');
    for part in value.split_inclusive('"') {
        out.push_str(part);
        if part.ends_with('"') {
            out.push('"');
        }
    }
    out.push('"');
}

impl CsvEncoder {
    fn record(&mut self, columns: &[Column<'_>], row: usize, out: &mut String) {
        let delimiter = self.options.delimiter.as_char();
        for (index, column) in columns.iter().enumerate() {
            if index > 0 {
                out.push(delimiter);
            }
            self.field(&column.cell(row), out);
        }
        out.push_str("\r\n");
    }
}

fn push_header(schema: &Schema, delimiter: char, out: &mut String) {
    for (index, field) in schema.fields().iter().enumerate() {
        if index > 0 {
            out.push(delimiter);
        }
        push_field(field.name(), delimiter, out);
    }
    out.push_str("\r\n");
}

impl Encode for CsvEncoder {
    fn begin(&mut self, schema: &Schema, out: &mut String) -> Result<(), ExportError> {
        if self.options.header {
            push_header(schema, self.options.delimiter.as_char(), out);
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
