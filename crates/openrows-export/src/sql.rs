// SPDX-License-Identifier: Apache-2.0
//! SQL `INSERT` statements with dialect-correct identifiers and literals.
//!
//! Strings: `PostgreSQL` doubles `'` and switches to an `E'…'` literal when
//! a backslash is present, so the text is right whatever
//! `standard_conforming_strings` says; `MySQL` backslash-escapes as its
//! default `sql_mode` expects; `SQLite` only doubles `'`. Binary is
//! `decode('…', 'hex')` (`PostgreSQL`) or `X'…'`. Booleans are `TRUE`/`FALSE`
//! except on `SQLite` (`1`/`0`). Non-finite floats become `'NaN'`/`'Infinity'`
//! on `PostgreSQL`, `±9e999` (infinity) on `SQLite` and NULL where the
//! dialect has no such value.
use crate::cell::{Cell, Column};
use crate::exporter::Encode;
use crate::text::{push_hex, push_plain};
use crate::{ExportError, InsertOptions};
use arrow_schema::Schema;
use openrows_sql::Dialect;

pub(crate) struct InsertEncoder {
    options: InsertOptions,
    /// `INSERT INTO "t" ("a", "b") VALUES`.
    head: String,
    /// Rows already in the open statement.
    open: usize,
    scratch: String,
}

impl InsertEncoder {
    pub(crate) fn new(options: InsertOptions) -> Self {
        Self {
            options,
            head: String::new(),
            open: 0,
            scratch: String::new(),
        }
    }

    /// One `(…)` value tuple, opening or closing a statement as needed.
    fn tuple(&mut self, columns: &[Column<'_>], row: usize, out: &mut String) {
        if self.open == 0 {
            out.push_str(&self.head);
            out.push_str("\n(");
        } else {
            out.push_str(",\n(");
        }
        for (index, column) in columns.iter().enumerate() {
            if index > 0 {
                out.push_str(", ");
            }
            push_literal(
                self.options.dialect,
                &column.cell(row),
                &mut self.scratch,
                out,
            );
        }
        out.push(')');
        self.open += 1;
        if self.open >= self.options.rows_per_statement.get() {
            out.push_str(";\n");
            self.open = 0;
        }
    }
}

fn quote(dialect: Dialect, name: &str) -> Result<String, ExportError> {
    dialect
        .quote_ident(name)
        .map_err(|_| ExportError::InvalidName(name.to_owned()))
}

impl Encode for InsertEncoder {
    fn begin(&mut self, schema: &Schema, _out: &mut String) -> Result<(), ExportError> {
        if schema.fields().is_empty() {
            return Err(ExportError::NoColumns);
        }
        let dialect = self.options.dialect;
        let columns = schema
            .fields()
            .iter()
            .map(|field| quote(dialect, field.name()))
            .collect::<Result<Vec<_>, _>>()?;
        self.head = format!(
            "INSERT INTO {} ({}) VALUES",
            quote(dialect, &self.options.table)?,
            columns.join(", ")
        );
        Ok(())
    }

    fn rows(&mut self, columns: &[Column<'_>], rows: usize, out: &mut String) {
        for row in 0..rows {
            self.tuple(columns, row, out);
        }
    }

    fn finish(&mut self, out: &mut String) {
        if self.open > 0 {
            out.push_str(";\n");
            self.open = 0;
        }
    }
}

/// Append `cell` as an SQL literal; `scratch` holds intermediate text.
pub(crate) fn push_literal(
    dialect: Dialect,
    cell: &Cell<'_>,
    scratch: &mut String,
    out: &mut String,
) {
    match cell {
        Cell::Null => out.push_str("NULL"),
        Cell::Bool(value) => out.push_str(match (dialect, value) {
            (Dialect::Sqlite | Dialect::SqlServer, true) => "1",
            (Dialect::Sqlite | Dialect::SqlServer, false) => "0",
            (_, true) => "TRUE",
            (_, false) => "FALSE",
        }),
        Cell::Int(_) | Cell::UInt(_) | Cell::Number(_) => push_plain(cell, out),
        Cell::F32(value) => push_float(dialect, f64::from(*value), cell, out),
        Cell::F64(value) => push_float(dialect, *value, cell, out),
        Cell::Binary(bytes) => push_binary(dialect, bytes, out),
        _ => {
            scratch.clear();
            push_plain(cell, scratch);
            push_string(dialect, scratch, out);
        }
    }
}

fn push_float(dialect: Dialect, value: f64, cell: &Cell<'_>, out: &mut String) {
    if value.is_finite() {
        push_plain(cell, out);
        return;
    }
    let negative = value < 0.0;
    match dialect {
        Dialect::Postgres if value.is_nan() => out.push_str("'NaN'"),
        Dialect::Postgres if negative => out.push_str("'-Infinity'"),
        Dialect::Postgres => out.push_str("'Infinity'"),
        Dialect::Sqlite if value.is_infinite() => {
            out.push_str(if negative { "-9e999" } else { "9e999" });
        }
        Dialect::ClickHouse => out.push_str(if value.is_nan() {
            "toFloat64('nan')"
        } else if negative {
            "toFloat64('-inf')"
        } else {
            "toFloat64('inf')"
        }),
        _ => out.push_str("NULL"),
    }
}

fn push_binary(dialect: Dialect, bytes: &[u8], out: &mut String) {
    if dialect == Dialect::Postgres {
        out.push_str("decode('");
        push_hex(bytes, out);
        out.push_str("', 'hex')");
    } else if dialect == Dialect::ClickHouse {
        out.push_str("unhex('");
        push_hex(bytes, out);
        out.push_str("')");
    } else if dialect == Dialect::SqlServer {
        out.push_str("0x");
        push_hex(bytes, out);
    } else {
        out.push_str("X'");
        push_hex(bytes, out);
        out.push('\'');
    }
}

/// Append `text` as a string literal of `dialect`.
pub(crate) fn push_string(dialect: Dialect, text: &str, out: &mut String) {
    match dialect {
        Dialect::MySql => push_mysql_string(text, out, false),
        Dialect::ClickHouse => push_mysql_string(text, out, true),
        Dialect::Postgres if text.contains('\\') => {
            out.push_str("E'");
            for c in text.chars() {
                match c {
                    '\'' => out.push_str("''"),
                    '\\' => out.push_str("\\\\"),
                    c => out.push(c),
                }
            }
            out.push('\'');
        }
        // Unicode literals, so text outside the database code page survives.
        Dialect::SqlServer => {
            out.push_str("N'");
            out.push_str(&text.replace('\'', "''"));
            out.push('\'');
        }
        _ => {
            out.push('\'');
            out.push_str(&text.replace('\'', "''"));
            out.push('\'');
        }
    }
}

/// `MySQL` string literal for the default `sql_mode` (backslash escapes on).
fn push_mysql_string(text: &str, out: &mut String, clickhouse: bool) {
    out.push('\'');
    for c in text.chars() {
        match c {
            '\'' => out.push_str("\\'"),
            '\\' => out.push_str("\\\\"),
            '\0' => out.push_str("\\0"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\u{1a}' => out.push_str(if clickhouse { "\\x1a" } else { "\\Z" }),
            c => out.push(c),
        }
    }
    out.push('\'');
}
