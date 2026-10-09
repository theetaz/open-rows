// SPDX-License-Identifier: Apache-2.0
//! The temporary-file lifecycle and the streamed pipeline.
use super::{TestResult, is_unsupported, texts};
use crate::{CsvOptions, Delimiter, ExportError, Exporter, Format, Message, drain, write_all};
use arrow_array::{Array, ArrayRef, DurationSecondArray, RecordBatch};
use arrow_schema::{Field, Schema};
use std::convert::Infallible;
use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::sync_channel;

fn csv() -> Format {
    Format::Csv(CsvOptions {
        delimiter: Delimiter::Comma,
        header: false,
        ..CsvOptions::default()
    })
}

fn entries(directory: &Path) -> TestResult<Vec<String>> {
    let mut names = std::fs::read_dir(directory)?
        .map(|entry| Ok(entry?.file_name().to_string_lossy().into_owned()))
        .collect::<Result<Vec<_>, std::io::Error>>()?;
    names.sort();
    Ok(names)
}

#[test]
fn the_file_appears_only_when_finished_and_replaces_the_old_one() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("out.csv");
    std::fs::write(&path, "old")?;
    let batch = texts(&[Some("new")])?;
    let mut exporter = Exporter::create(&path, &csv())?;
    exporter.begin(batch.schema())?;
    exporter.write(&batch)?;
    let during = entries(directory.path())?;
    assert_eq!(during.len(), 2);
    let temporary = |name: &String| {
        name.starts_with(".out.csv.")
            && Path::new(name)
                .extension()
                .is_some_and(|extension| extension == "part")
    };
    assert!(during.iter().any(temporary));
    assert_eq!(std::fs::read_to_string(&path)?, "old");
    assert_eq!(exporter.finish()?, 1);
    assert_eq!(entries(directory.path())?, ["out.csv"]);
    assert_eq!(std::fs::read_to_string(&path)?, "new\r\n");
    Ok(())
}

#[test]
fn a_cancelled_export_leaves_nothing_and_keeps_the_old_file() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("keep.csv");
    std::fs::write(&path, "old")?;
    let batch = texts(&[Some("a"), Some("b")])?;
    let batches = [batch.clone(), batch.clone()];
    let seen = std::cell::Cell::new(0);
    let result = write_all(
        Exporter::create(&path, &csv())?,
        (batch.schema(), batches.map(Ok::<_, Infallible>)),
        || seen.get() >= 2,
        |rows| seen.set(rows),
    );
    assert!(matches!(result, Err(ExportError::Aborted)));
    assert_eq!(entries(directory.path())?, ["keep.csv"]);
    assert_eq!(std::fs::read_to_string(&path)?, "old");
    Ok(())
}

#[test]
fn a_batch_that_cannot_be_read_fails_the_export_and_leaves_nothing() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("read.csv");
    let batch = texts(&[Some("a")])?;
    let batches = [
        Ok(batch.clone()),
        Err(std::io::Error::other("spill file gone")),
        Ok(batch.clone()),
    ];
    let result = write_all(
        Exporter::create(&path, &csv())?,
        (batch.schema(), batches),
        || false,
        |_| {},
    );
    assert!(matches!(result, Err(ExportError::Read(_))), "{result:?}");
    assert!(entries(directory.path())?.is_empty());
    Ok(())
}

#[test]
fn drain_commits_on_finish_and_aborts_when_the_producer_disappears() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("stream.csv");
    let batch = texts(&[Some("x"), None])?;
    let (sender, receiver) = sync_channel(4);
    sender.send(Message::Begin(batch.schema()))?;
    sender.send(Message::Batch(batch.clone()))?;
    sender.send(Message::Batch(batch.clone()))?;
    sender.send(Message::Finish)?;
    let mut progress = Vec::new();
    let rows = drain(Exporter::create(&path, &csv())?, receiver, |rows| {
        progress.push(rows);
    })?;
    assert_eq!((rows, progress), (4, vec![2, 4]));
    assert_eq!(std::fs::read_to_string(&path)?, "x\r\n\r\nx\r\n\r\n");

    let other = directory.path().join("dropped.csv");
    let (sender, receiver) = sync_channel(4);
    sender.send(Message::Begin(batch.schema()))?;
    sender.send(Message::Batch(batch))?;
    drop(sender);
    let result = drain(Exporter::create(&other, &csv())?, receiver, |_| {});
    assert!(matches!(result, Err(ExportError::Aborted)));
    assert_eq!(entries(directory.path())?, ["stream.csv"]);
    Ok(())
}

#[test]
fn finishing_without_a_result_or_writing_before_begin_fails_cleanly() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("none.csv");
    let exporter = Exporter::create(&path, &csv())?;
    assert!(matches!(exporter.finish(), Err(ExportError::NoResult)));
    let mut exporter = Exporter::create(&path, &csv())?;
    let batch = texts(&[Some("a")])?;
    assert!(matches!(
        exporter.write(&batch),
        Err(ExportError::SchemaMismatch)
    ));
    drop(exporter);
    assert!(entries(directory.path())?.is_empty());
    Ok(())
}

#[test]
fn a_continued_result_with_the_same_columns_keeps_writing() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("continued.csv");
    let numbers: ArrayRef = Arc::new(arrow_array::Int64Array::from(vec![1]));
    let first = RecordBatch::try_from_iter([("v", numbers)])?;
    let text = texts(&[Some("infinity")])?;
    let mut exporter = Exporter::create(
        &path,
        &Format::Csv(CsvOptions {
            delimiter: Delimiter::Comma,
            header: true,
            ..CsvOptions::default()
        }),
    )?;
    exporter.begin(first.schema())?;
    exporter.write(&first)?;
    exporter.begin(text.schema())?;
    exporter.write(&text)?;
    let renamed = Arc::new(Schema::new(vec![Field::new(
        "w",
        arrow_schema::DataType::Utf8,
        true,
    )]));
    assert!(matches!(
        exporter.begin(renamed),
        Err(ExportError::SchemaMismatch)
    ));
    assert_eq!(exporter.finish()?, 2);
    assert_eq!(std::fs::read_to_string(&path)?, "v\r\n1\r\ninfinity\r\n");
    Ok(())
}

#[test]
fn unsupported_types_are_refused_before_any_row_is_written() -> TestResult {
    let directory = tempfile::tempdir()?;
    let durations = DurationSecondArray::from(vec![1]);
    let field = Field::new("l", durations.data_type().clone(), true);
    let column: ArrayRef = Arc::new(durations);
    let batch = RecordBatch::try_new(Arc::new(Schema::new(vec![field])), vec![column])?;
    let mut exporter = Exporter::create(&directory.path().join("l.csv"), &csv())?;
    let error = exporter
        .begin(batch.schema())
        .err()
        .ok_or("accepted a duration")?;
    assert!(is_unsupported(&error), "{error}");
    drop(exporter);
    assert!(entries(directory.path())?.is_empty());
    Ok(())
}

#[test]
fn a_destination_without_a_file_name_or_directory_is_an_error() -> TestResult {
    assert!(matches!(
        Exporter::create(Path::new("/"), &csv()),
        Err(ExportError::InvalidPath)
    ));
    let directory = tempfile::tempdir()?;
    let missing = directory.path().join("missing").join("out.csv");
    assert!(matches!(
        Exporter::create(&missing, &csv()),
        Err(ExportError::Io(_))
    ));
    Ok(())
}
