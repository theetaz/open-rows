// SPDX-License-Identifier: Apache-2.0
//! A GitHub-flavoured Markdown table, as the grid's Copy as ▸ Markdown
//! writes it: `\` and `|` are escaped, line breaks become `<br>`, NULL is
//! `NULL` and numeric columns are right-aligned.
use crate::ExportError;
use crate::cell::{Cell, Column};
use crate::exporter::Encode;
use crate::text::push_plain;
use arrow_schema::{Field, Schema};
use openrows_driver_api::metadata::{DECIMAL_EXTENSION, EXTENSION_NAME};

#[derive(Default)]
pub(crate) struct MarkdownEncoder {
    scratch: String,
}

/// Append `value` with Markdown table syntax escaped.
fn push_escaped(value: &str, out: &mut String) {
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => out.push_str("\\\\"),
            '|' => out.push_str("\\|"),
            '\r' => {
                chars.next_if_eq(&'\n');
                out.push_str("<br>");
            }
            '\n' => out.push_str("<br>"),
            c => out.push(c),
        }
    }
}

fn is_numeric(field: &Field) -> bool {
    field.data_type().is_numeric()
        || field.metadata().get(EXTENSION_NAME).map(String::as_str) == Some(DECIMAL_EXTENSION)
}

impl MarkdownEncoder {
    fn cell(&mut self, cell: &Cell<'_>, out: &mut String) {
        if *cell == Cell::Null {
            out.push_str("NULL");
            return;
        }
        self.scratch.clear();
        push_plain(cell, &mut self.scratch);
        push_escaped(&self.scratch, out);
    }
}

impl Encode for MarkdownEncoder {
    fn begin(&mut self, schema: &Schema, out: &mut String) -> Result<(), ExportError> {
        out.push('|');
        for field in schema.fields() {
            out.push(' ');
            push_escaped(field.name(), out);
            out.push_str(" |");
        }
        out.push_str("\n|");
        for field in schema.fields() {
            out.push_str(if is_numeric(field) {
                " ---: |"
            } else {
                " --- |"
            });
        }
        out.push('\n');
        Ok(())
    }

    fn rows(&mut self, columns: &[Column<'_>], rows: usize, out: &mut String) {
        for row in 0..rows {
            out.push('|');
            for column in columns {
                out.push(' ');
                self.cell(&column.cell(row), out);
                out.push_str(" |");
            }
            out.push('\n');
        }
    }

    fn finish(&mut self, _out: &mut String) {}
}
