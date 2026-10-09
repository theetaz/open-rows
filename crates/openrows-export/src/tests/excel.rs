// SPDX-License-Identifier: Apache-2.0
//! Workbooks read back with `calamine`: typed cells, sheet splitting and a
//! sheet per table.
use super::{TestResult, mixed, texts};
use crate::{ExcelOptions, Exporter, Format};
use calamine::{Data, Reader, Xlsx, open_workbook};
use std::path::Path;

fn options(sheet: &str) -> ExcelOptions {
    ExcelOptions {
        sheet: sheet.to_owned(),
        header: true,
    }
}

fn sheets(path: &Path) -> TestResult<Vec<(String, Vec<Vec<Data>>)>> {
    let mut workbook: Xlsx<_> = open_workbook(path)?;
    let mut sheets = Vec::new();
    for name in workbook.sheet_names() {
        let range = workbook.worksheet_range(&name)?;
        sheets.push((name, range.rows().map(<[Data]>::to_vec).collect()));
    }
    Ok(sheets)
}

#[test]
fn cells_keep_their_types() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("typed.xlsx");
    let batch = mixed()?;
    let mut exporter = Exporter::create(&path, &Format::Excel(options("Result")))?;
    exporter.begin(batch.schema())?;
    exporter.write(&batch)?;
    assert_eq!(exporter.finish()?, 2);
    let sheets = sheets(&path)?;
    let [(name, rows)] = sheets.as_slice() else {
        return Err("expected one sheet".into());
    };
    assert_eq!(name, "Result");
    assert_eq!(rows[0][0], Data::String("id".to_owned()));
    let values = &rows[1];
    assert_eq!(values[0], Data::Float(-7.0));
    // Beyond 2^53: text, so no digit is lost.
    assert_eq!(values[1], Data::String("18446744073709551615".to_owned()));
    assert_eq!(values[2], Data::Bool(true));
    assert_eq!(values[3], Data::Float(0.5));
    assert_eq!(values[4], Data::Float(-12.34));
    assert_eq!(values[5], Data::String("it's \"ü\", ok".to_owned()));
    assert_eq!(values[7], Data::String("\\x00ff10".to_owned()));
    let Data::DateTime(day) = &values[9] else {
        return Err(format!("day is {:?}", values[9]).into());
    };
    assert!((day.as_f64() - 45_351.0).abs() < 1e-9);
    assert!(matches!(&values[10], Data::DateTime(_)));
    // The all-NULL row writes no cells.
    assert_eq!(rows.len(), 2);
    Ok(())
}

#[test]
fn long_tables_continue_on_extra_sheets_and_tables_get_their_own() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("split.xlsx");
    let batch = texts(&[Some("a"), Some("b"), Some("c"), Some("d"), Some("e")])?;
    let mut exporter = Exporter::excel_with_sheet_rows(&path, options("ignored"), 3)?;
    exporter.start_table("first")?;
    exporter.begin(batch.schema())?;
    exporter.write(&batch)?;
    exporter.start_table("second")?;
    exporter.begin(batch.schema())?;
    exporter.write(&batch.slice(0, 1))?;
    assert_eq!(exporter.finish()?, 6);
    let sheets = sheets(&path)?;
    let names: Vec<&str> = sheets.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["first", "first (2)", "first (3)", "second"]);
    let text = |rows: &Vec<Vec<Data>>| -> Vec<String> {
        rows.iter().map(|row| row[0].to_string()).collect()
    };
    assert_eq!(text(&sheets[0].1), ["v", "a", "b"]);
    assert_eq!(text(&sheets[1].1), ["v", "c", "d"]);
    assert_eq!(text(&sheets[2].1), ["v", "e"]);
    assert_eq!(text(&sheets[3].1), ["v", "a"]);
    Ok(())
}

#[test]
fn text_formats_hold_one_table() -> TestResult {
    let directory = tempfile::tempdir()?;
    let mut exporter = Exporter::create(&directory.path().join("x.md"), &Format::Markdown)?;
    assert!(exporter.start_table("t").is_err());
    Ok(())
}
