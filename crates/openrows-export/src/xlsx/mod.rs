// SPDX-License-Identifier: Apache-2.0
//! Excel `.xlsx` workbooks written in constant-memory mode: each row goes
//! to a temporary file as soon as the next one starts, and the workbook is
//! zipped into the export file at the end.
//!
//! Every table gets its own sheet, with the column names as a bold first
//! row. A table longer than a sheet (1,048,576 rows) continues on extra
//! sheets named `name (2)`, `name (3)`…, each with the header again.
mod cells;

use crate::cell::Column;
use crate::exporter::WriteBatches;
use crate::{ExcelOptions, ExportError};
use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use cells::CellWriter;
use rust_xlsxwriter::{Format as CellFormat, Workbook, XlsxError};
use std::collections::HashSet;
use std::fs::File;

/// Rows of one Excel sheet.
const SHEET_ROWS: u32 = 1_048_576;
/// Columns of one Excel sheet.
const SHEET_COLUMNS: usize = 16_384;
/// Longest sheet name, in characters.
const MAX_NAME: usize = 31;

pub(crate) struct ExcelWriter {
    workbook: Workbook,
    file: Option<File>,
    header: bool,
    /// Name of the next table's sheet.
    next_name: String,
    /// The current table's name, column names and sheet count.
    table: String,
    columns: Vec<String>,
    part: usize,
    /// Lower-case names of the sheets so far (Excel compares them so).
    names: HashSet<String>,
    sheets: usize,
    /// Next row of the current sheet.
    row: u32,
    sheet_rows: u32,
    bold: CellFormat,
    cells: CellWriter,
}

impl From<XlsxError> for ExportError {
    fn from(error: XlsxError) -> Self {
        Self::Excel(error.to_string())
    }
}

impl ExcelWriter {
    pub(crate) fn new(file: File, options: ExcelOptions) -> Self {
        Self {
            workbook: Workbook::new(),
            file: Some(file),
            header: options.header,
            next_name: options.sheet,
            table: String::new(),
            columns: Vec::new(),
            part: 0,
            names: HashSet::new(),
            sheets: 0,
            row: 0,
            sheet_rows: SHEET_ROWS,
            bold: CellFormat::new().set_bold(),
            cells: CellWriter::new(),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_sheet_rows(mut self, rows: u32) -> Self {
        self.sheet_rows = rows;
        self
    }

    /// Add the next sheet of the current table and write its header.
    fn open_sheet(&mut self) -> Result<(), ExportError> {
        self.part += 1;
        let name = unique_name(&self.table, self.part, &mut self.names);
        let sheet = self.workbook.add_worksheet_with_constant_memory();
        sheet.set_name(name)?;
        self.sheets += 1;
        self.row = 0;
        if self.header {
            for (index, name) in self.columns.iter().enumerate() {
                let column = u16::try_from(index).map_err(|_| too_wide())?;
                sheet.write_string_with_format(0, column, name, &self.bold)?;
            }
            self.row = 1;
        }
        Ok(())
    }
}

fn too_wide() -> ExportError {
    ExportError::Excel(format!("a sheet holds at most {SHEET_COLUMNS} columns"))
}

impl WriteBatches for ExcelWriter {
    fn begin(&mut self, schema: &SchemaRef) -> Result<(), ExportError> {
        if schema.fields().len() > SHEET_COLUMNS {
            return Err(too_wide());
        }
        self.columns = schema.fields().iter().map(|f| f.name().clone()).collect();
        self.table = std::mem::take(&mut self.next_name);
        self.part = 0;
        self.open_sheet()
    }

    fn resume(&mut self, _schema: &SchemaRef) -> Result<(), ExportError> {
        Ok(())
    }

    fn table(&mut self, name: &str) -> Result<(), ExportError> {
        name.clone_into(&mut self.next_name);
        Ok(())
    }

    fn write(&mut self, columns: &[Column<'_>], batch: &RecordBatch) -> Result<(), ExportError> {
        for row in 0..batch.num_rows() {
            if self.row >= self.sheet_rows {
                self.open_sheet()?;
            }
            let sheet = self.workbook.worksheet_from_index(self.sheets - 1)?;
            for (index, column) in columns.iter().enumerate() {
                let at = (self.row, u16::try_from(index).map_err(|_| too_wide())?);
                self.cells.write(sheet, at, &column.cell(row))?;
            }
            self.row += 1;
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), ExportError> {
        let file = self.file.take().ok_or(ExportError::Aborted)?;
        self.workbook.save_to_writer(file)?;
        Ok(())
    }
}

/// A valid, unused sheet name: `[]:*?/\` removed, no edge apostrophes,
/// at most 31 characters with the ` (n)` suffix of later parts.
fn unique_name(base: &str, part: usize, used: &mut HashSet<String>) -> String {
    let cleaned: String = base
        .chars()
        .filter(|c| !matches!(c, '[' | ']' | ':' | '*' | '?' | '/' | '\\'))
        .collect();
    let cleaned = cleaned.trim().trim_matches('\'');
    let base = if cleaned.is_empty() { "Sheet" } else { cleaned };
    let mut number = part;
    loop {
        let suffix = if number > 1 {
            format!(" ({number})")
        } else {
            String::new()
        };
        let keep = MAX_NAME - suffix.chars().count();
        let name: String = base.chars().take(keep).chain(suffix.chars()).collect();
        if used.insert(name.to_lowercase()) {
            return name;
        }
        number += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::unique_name;
    use std::collections::HashSet;

    #[test]
    fn sheet_names_are_valid_and_unique() {
        let mut used = HashSet::new();
        assert_eq!(unique_name("orders", 1, &mut used), "orders");
        assert_eq!(unique_name("orders", 2, &mut used), "orders (2)");
        assert_eq!(unique_name("ORDERS", 1, &mut used), "ORDERS (3)");
        assert_eq!(unique_name("'a/b:c'", 1, &mut used), "abc");
        assert_eq!(unique_name("  ", 1, &mut used), "Sheet");
        let long = unique_name(&"x".repeat(40), 2, &mut used);
        assert_eq!(long.chars().count(), 31);
        assert!(long.ends_with(" (2)"));
    }
}
