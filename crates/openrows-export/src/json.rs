// SPDX-License-Identifier: Apache-2.0
//! JSON rows: one object per row keyed by column name, either inside one
//! array (compact or indented) or one per line (NDJSON).
//!
//! Integers, decimals and finite floats are numbers; JSON columns are
//! embedded as documents; everything else is a string in its canonical text
//! form (non-finite floats too, since JSON has no NaN or Infinity).
use crate::cell::{Cell, Column};
use crate::exporter::Encode;
use crate::text::push_plain;
use crate::{ExportError, JsonLayout};
use arrow_schema::Schema;
use std::fmt::Write as _;

pub(crate) struct JsonEncoder {
    layout: JsonLayout,
    /// `"name":` per column, escaped once.
    keys: Vec<String>,
    rows: u64,
    scratch: String,
}

impl JsonEncoder {
    pub(crate) fn new(layout: JsonLayout) -> Self {
        Self {
            layout,
            keys: Vec::new(),
            rows: 0,
            scratch: String::new(),
        }
    }

    fn value(&mut self, cell: &Cell<'_>, out: &mut String) {
        push_value(cell, out, &mut self.scratch);
    }

    /// One row object, with its array separator or line end.
    fn object(&mut self, columns: &[Column<'_>], row: usize, out: &mut String) {
        let pretty = self.layout == JsonLayout::Pretty;
        let separator = match (self.layout, self.rows) {
            (JsonLayout::Lines, _) => "",
            (_, 0) => "\n",
            _ => ",\n",
        };
        out.push_str(separator);
        out.push_str(if pretty { "  {" } else { "{" });
        for (index, column) in columns.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            if pretty {
                out.push_str("\n    ");
            }
            out.push_str(self.keys.get(index).map_or("", String::as_str));
            if pretty {
                out.push(' ');
            }
            self.value(&column.cell(row), out);
        }
        out.push_str(if pretty { "\n  }" } else { "}" });
        if self.layout == JsonLayout::Lines {
            out.push('\n');
        }
        self.rows += 1;
    }
}

/// Encode one scalar or nested document without converting exact integers to floats.
pub(crate) fn push_value(cell: &Cell<'_>, out: &mut String, scratch: &mut String) {
    match cell {
        Cell::Null => out.push_str("null"),
        Cell::Bool(_) | Cell::Int(_) | Cell::UInt(_) | Cell::Number(_) | Cell::Json(_) => {
            push_plain(cell, out);
        }
        Cell::F32(value) if value.is_finite() => push_plain(cell, out),
        Cell::F64(value) if value.is_finite() => push_plain(cell, out),
        _ => {
            scratch.clear();
            push_plain(cell, scratch);
            push_string(scratch, out);
        }
    }
}

/// Append `value` as a JSON string literal.
pub(crate) fn push_string(value: &str, out: &mut String) {
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if c < ' ' => {
                // Writing to a `String` cannot fail.
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

impl Encode for JsonEncoder {
    fn begin(&mut self, schema: &Schema, out: &mut String) -> Result<(), ExportError> {
        self.keys = schema
            .fields()
            .iter()
            .map(|field| {
                let mut key = String::new();
                push_string(field.name(), &mut key);
                key.push(':');
                key
            })
            .collect();
        if self.layout != JsonLayout::Lines {
            out.push('[');
        }
        Ok(())
    }

    fn rows(&mut self, columns: &[Column<'_>], rows: usize, out: &mut String) {
        for row in 0..rows {
            self.object(columns, row, out);
        }
    }

    fn finish(&mut self, out: &mut String) {
        if self.layout != JsonLayout::Lines {
            out.push_str(if self.rows == 0 { "]\n" } else { "\n]\n" });
        }
    }
}
