// SPDX-License-Identifier: Apache-2.0
//! Parquet files read back with the `parquet` crate's Arrow reader.
use super::{TestResult, mixed};
use crate::{Exporter, Format, ParquetCompression};
use arrow_array::cast::AsArray;
use arrow_array::types::Int64Type;
use arrow_array::{ArrayRef, IntervalMonthDayNanoArray, RecordBatch};
use arrow_schema::DataType;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use std::path::Path;
use std::sync::Arc;

fn read(path: &Path) -> TestResult<Vec<RecordBatch>> {
    let reader = ParquetRecordBatchReaderBuilder::try_new(std::fs::File::open(path)?)?.build()?;
    Ok(reader.collect::<Result<Vec<_>, _>>()?)
}

fn write(path: &Path, compression: ParquetCompression, batches: &[RecordBatch]) -> TestResult<u64> {
    let mut exporter = Exporter::create(path, &Format::Parquet(compression))?;
    exporter.begin(batches[0].schema())?;
    for batch in batches {
        exporter.write(batch)?;
    }
    Ok(exporter.finish()?)
}

#[test]
fn every_column_round_trips_with_its_arrow_type() -> TestResult {
    let directory = tempfile::tempdir()?;
    let batch = mixed()?;
    for compression in [ParquetCompression::Snappy, ParquetCompression::Uncompressed] {
        let path = directory.path().join(format!("{compression:?}.parquet"));
        assert_eq!(
            write(&path, compression, &[batch.clone(), batch.clone()])?,
            4
        );
        let back = read(&path)?;
        let rows: usize = back.iter().map(RecordBatch::num_rows).sum();
        assert_eq!(rows, 4);
        let first = &back[0];
        for (field, original) in first.schema().fields().iter().zip(batch.schema().fields()) {
            assert_eq!(field.name(), original.name());
            assert_eq!(field.data_type(), original.data_type(), "{}", field.name());
        }
        for (column, original) in first.columns().iter().zip(batch.columns()) {
            assert_eq!(column.slice(0, 2).as_ref(), original.as_ref());
        }
        assert_eq!(first.column(0).as_primitive::<Int64Type>().value(0), -7);
    }
    Ok(())
}

#[test]
fn intervals_are_written_as_text() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("interval.parquet");
    let interval = IntervalMonthDayNanoArray::from(vec![
        Some(arrow_array::types::IntervalMonthDayNano::new(
            14,
            3,
            3_600_000_000_000,
        )),
        None,
    ]);
    let column: ArrayRef = Arc::new(interval);
    let batch = RecordBatch::try_from_iter([("span", column)])?;
    write(&path, ParquetCompression::Snappy, &[batch])?;
    let back = read(&path)?;
    let text = back[0].column(0);
    assert_eq!(text.data_type(), &DataType::Utf8);
    assert_eq!(
        text.as_string::<i32>().value(0),
        "1 year 2 mons 3 days 01:00:00"
    );
    assert!(text.is_null(1));
    Ok(())
}
