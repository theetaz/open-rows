// SPDX-License-Identifier: Apache-2.0
//! Parsing real files: detection, quoting, embedded newlines, BOMs,
//! malformed rows, JSON shapes and streaming a large file.
use std::io::Write;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{
    FileFormat, ImportError, Inspection, Item, ParseOptions, RowError, SampleLimits, TypeCategory,
    inspect, stream,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const LIMITS: SampleLimits = SampleLimits {
    preview: 10,
    sample: 100,
};

fn file(dir: &tempfile::TempDir, name: &str, bytes: &[u8]) -> TestResult<PathBuf> {
    let path = dir.path().join(name);
    std::fs::File::create(&path)?.write_all(bytes)?;
    Ok(path)
}

fn names(inspection: &Inspection) -> Vec<&str> {
    inspection
        .columns
        .iter()
        .map(|column| column.name.as_str())
        .collect()
}

fn categories(inspection: &Inspection) -> Vec<TypeCategory> {
    inspection
        .columns
        .iter()
        .map(|column| column.stats.category())
        .collect()
}

fn cells(inspection: &Inspection, row: usize) -> Vec<Option<&str>> {
    inspection.rows[row]
        .cells
        .iter()
        .map(Option::as_deref)
        .collect()
}

fn read_all(path: &Path, options: &ParseOptions, columns: &[String]) -> TestResult<Vec<Item>> {
    let mut items = Vec::new();
    stream(path, (options, columns), Arc::default(), &mut |item| {
        items.push(item);
        ControlFlow::Continue(())
    })?;
    Ok(items)
}

#[test]
fn csv_with_bom_quotes_and_embedded_newlines_is_detected_and_typed() -> TestResult {
    let dir = tempfile::tempdir()?;
    let text = "\u{FEFF}id,name,score,joined,active\n\
        1,\"Ada, Countess\",1.5,2024-01-02,true\n\
        2,\"multi\nline \"\"quoted\"\"\",,2024-01-03 10:00,false\n\
        3,plain,-2,2024-01-04,TRUE\n";
    let path = file(&dir, "people.csv", text.as_bytes())?;
    let found = inspect(&path, None, LIMITS)?;
    assert_eq!(found.options.format, FileFormat::Delimited);
    assert_eq!(found.options.delimiter, b',');
    assert!(found.options.header);
    assert_eq!(found.options.encoding, "utf-8");
    assert_eq!(names(&found), ["id", "name", "score", "joined", "active"]);
    assert_eq!(
        categories(&found),
        [
            TypeCategory::Integer,
            TypeCategory::Text,
            TypeCategory::Decimal,
            TypeCategory::Timestamp,
            TypeCategory::Boolean
        ]
    );
    assert!(found.columns[2].stats.nullable);
    assert_eq!(found.rows.len(), 3);
    assert_eq!(found.rows[1].line, 3);
    assert_eq!(
        cells(&found, 1),
        [
            Some("2"),
            Some("multi\nline \"quoted\""),
            None,
            Some("2024-01-03 10:00"),
            Some("false")
        ]
    );
    assert_eq!(
        found.rows[2].line, 5,
        "line numbers count embedded newlines"
    );
    Ok(())
}

#[test]
fn malformed_rows_are_reported_with_line_numbers() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = file(&dir, "bad.csv", b"a,b\n1,2\n3\n4,5,6\n7,8\n")?;
    let found = inspect(&path, None, LIMITS)?;
    assert_eq!(found.rows.len(), 2);
    assert_eq!(
        found.errors,
        [
            RowError::row(3, "expected 2 fields but found 1"),
            RowError::row(4, "expected 2 fields but found 3"),
        ]
    );
    // An unterminated quote swallows the rest of the file into one field.
    let open = file(&dir, "open.csv", b"a,b\n1,\"never closed\n2,3\n")?;
    let options = inspect(&open, None, LIMITS)?.options;
    let items = read_all(&open, &options, &["a".to_owned(), "b".to_owned()])?;
    assert_eq!(items.len(), 1);
    Ok(())
}

#[test]
fn tsv_semicolon_and_headerless_files_are_detected() -> TestResult {
    let dir = tempfile::tempdir()?;
    let tsv = file(&dir, "t.tsv", b"x\ty\n1\t2\n")?;
    assert_eq!(inspect(&tsv, None, LIMITS)?.options.delimiter, b'\t');
    let semi = file(&dir, "s.txt", b"a;b\n\"1,5\";2\n\"2,5\";3\n")?;
    assert_eq!(inspect(&semi, None, LIMITS)?.options.delimiter, b';');
    let bare = file(&dir, "n.csv", b"1,Ada\n2,Grace\n")?;
    let found = inspect(&bare, None, LIMITS)?;
    assert!(!found.options.header);
    assert_eq!(names(&found), ["column_1", "column_2"]);
    assert_eq!(found.rows.len(), 2);
    let blank = file(&dir, "h.csv", b"id,,id\n1,2,3\n")?;
    let options = ParseOptions {
        header: true,
        ..ParseOptions::default()
    };
    assert_eq!(
        names(&inspect(&blank, Some(options), LIMITS)?),
        ["id", "column_2", "id_2"]
    );
    Ok(())
}

#[test]
fn legacy_and_utf16_encodings_are_decoded() -> TestResult {
    let dir = tempfile::tempdir()?;
    let latin = file(&dir, "latin.csv", b"name\ncaf\xE9\n")?;
    let found = inspect(&latin, None, LIMITS)?;
    assert_eq!(found.options.encoding, "windows-1252");
    assert_eq!(cells(&found, 0), [Some("café")]);
    let utf16: Vec<u8> = [0xFF, 0xFE]
        .into_iter()
        .chain("id,name\n1,Zoë\n".encode_utf16().flat_map(u16::to_le_bytes))
        .collect();
    let path = file(&dir, "wide.csv", &utf16)?;
    let found = inspect(&path, None, LIMITS)?;
    assert_eq!(found.options.encoding, "utf-16le");
    assert_eq!(cells(&found, 0), [Some("1"), Some("Zoë")]);
    let unknown = ParseOptions {
        encoding: "klingon".to_owned(),
        ..ParseOptions::default()
    };
    assert!(matches!(
        inspect(&path, Some(unknown), LIMITS),
        Err(ImportError::UnknownEncoding(_))
    ));
    Ok(())
}

#[test]
fn json_arrays_keep_key_order_exact_numbers_and_nested_text() -> TestResult {
    let dir = tempfile::tempdir()?;
    let text = br#"[
        {"id": 1, "price": 12345678901234567890.125, "tags": {"b": 1, "a": [true]}},
        {"id": 2, "name": "B\u00e9", "price": null},
        3,
        {"name": "", "id": 4}
    ]"#;
    let path = file(&dir, "rows.json", text)?;
    let found = inspect(&path, None, LIMITS)?;
    assert_eq!(found.options.format, FileFormat::JsonArray);
    assert_eq!(names(&found), ["id", "price", "tags", "name"]);
    assert_eq!(
        cells(&found, 0),
        [
            Some("1"),
            Some("12345678901234567890.125"),
            Some(r#"{"b": 1, "a": [true]}"#),
            None
        ]
    );
    assert_eq!(cells(&found, 1), [Some("2"), None, None, Some("Bé")]);
    assert_eq!(found.errors.len(), 1);
    assert_eq!(found.errors[0].line, 3);
    assert_eq!(
        cells(&found, 2),
        [Some("4"), None, None, None],
        "empty strings are NULL"
    );
    assert_eq!(categories(&found)[2], TypeCategory::Json);
    let broken = file(&dir, "broken.json", b"[{\"a\": 1}, {\"a\": }]")?;
    let options = inspect(&broken, None, LIMITS);
    assert!(matches!(options, Err(ImportError::Json { line: 1, .. })));
    Ok(())
}

#[test]
fn ndjson_skips_blank_lines_and_reports_bad_ones() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = file(
        &dir,
        "rows.ndjson",
        b"{\"a\": 1}\n\n{oops}\n[1]\n{\"a\": 2, \"b\": \"x\"}\n",
    )?;
    let found = inspect(&path, None, LIMITS)?;
    assert_eq!(found.options.format, FileFormat::JsonLines);
    assert_eq!(names(&found), ["a", "b"]);
    assert_eq!(found.rows.len(), 2);
    assert_eq!(found.rows[1].line, 5);
    let lines: Vec<u64> = found.errors.iter().map(|error| error.line).collect();
    assert_eq!(lines, [3, 4]);
    Ok(())
}

#[test]
fn large_files_stream_with_bounded_samples() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("large.csv");
    let mut writer = std::io::BufWriter::new(std::fs::File::create(&path)?);
    writeln!(writer, "id,label,amount")?;
    for row in 1..=200_000 {
        writeln!(writer, "{row},\"row {row}\",{}.5", row % 1000)?;
    }
    writer.flush()?;
    drop(writer);
    let size = std::fs::metadata(&path)?.len();
    let found = inspect(&path, None, LIMITS)?;
    assert_eq!((found.rows.len(), found.sampled), (10, 100));
    let columns: Vec<String> = found.columns.iter().map(|c| c.name.clone()).collect();
    let read = Arc::new(AtomicU64::new(0));
    let (mut rows, mut last) = (0_u64, 0);
    stream(
        &path,
        (&found.options, &columns),
        Arc::clone(&read),
        &mut |item| {
            if let Item::Record(record) = item {
                rows += 1;
                last = record.line;
            }
            ControlFlow::Continue(())
        },
    )?;
    assert_eq!((rows, last), (200_000, 200_001));
    assert_eq!(read.load(Ordering::Relaxed), size);
    let mut seen = 0;
    stream(
        &path,
        (&found.options, &columns),
        Arc::default(),
        &mut |_| {
            seen += 1;
            if seen == 5 {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        },
    )?;
    assert_eq!(seen, 5, "a stopped stream reads no further");
    Ok(())
}
