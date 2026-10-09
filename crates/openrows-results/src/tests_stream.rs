// SPDX-License-Identifier: Apache-2.0
//! Streaming appends, limits and release.
use super::*;
use arrow_array::{Int64Array, RecordBatch};
use arrow_ipc::reader::StreamReader;
use arrow_schema::{DataType, Field, Schema, SchemaRef};
use std::io::Cursor;
use std::sync::Arc;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn schema() -> SchemaRef {
    Arc::new(Schema::new(vec![Field::new("n", DataType::Int64, true)]))
}

fn batch(start: i64, rows: i64) -> Result<RecordBatch, arrow_schema::ArrowError> {
    RecordBatch::try_new(
        schema(),
        vec![Arc::new(Int64Array::from_iter_values(start..start + rows))],
    )
}

fn all(rows: usize) -> Window {
    Window {
        row_start: 0,
        row_count: rows,
        col_start: 0,
        col_count: 1,
    }
}

/// Every value of column 0 in an encoded window.
fn values(bytes: Vec<u8>) -> Result<Vec<i64>, Box<dyn std::error::Error>> {
    let mut out = Vec::new();
    for batch in StreamReader::try_new(Cursor::new(bytes), None)? {
        let batch = batch?;
        let column = batch
            .column(0)
            .as_any()
            .downcast_ref::<Int64Array>()
            .ok_or("not Int64")?;
        out.extend(column.iter().flatten());
    }
    Ok(out)
}

#[test]
fn appended_batches_are_readable_while_streaming() -> TestResult {
    let store = ResultStore::default();
    store.begin("r", schema());
    assert_eq!(store.rows("r"), Some((0, false)));
    assert!(values(store.window("r", all(10))?)?.is_empty());
    assert_eq!(store.append("r", batch(0, 3)?), Append::Continue);
    assert_eq!(values(store.window("r", all(10))?)?, vec![0, 1, 2]);
    assert_eq!(store.append("r", batch(3, 4)?), Append::Continue);
    let window = Window {
        row_start: 2,
        row_count: 3,
        ..all(0)
    };
    assert_eq!(values(store.window("r", window)?)?, vec![2, 3, 4]);
    assert_eq!(store.rows("r"), Some((7, false)));
    let (stored_schema, batches) = store.batches("r").ok_or("missing result")?;
    assert_eq!(stored_schema, schema());
    let rows: Vec<usize> = batches.iter().map(RecordBatch::num_rows).collect();
    assert_eq!(rows, [3, 4]);
    assert!(store.batches("unknown").is_none());
    Ok(())
}

#[test]
fn row_cap_cuts_the_batch_and_stops() -> TestResult {
    let store = ResultStore::with_limits(Limits {
        max_rows: 5,
        ..Limits::default()
    });
    store.begin("r", schema());
    assert_eq!(store.append("r", batch(0, 3)?), Append::Continue);
    assert_eq!(store.append("r", batch(3, 4)?), Append::Full);
    assert_eq!(store.rows("r"), Some((5, true)));
    assert_eq!(values(store.window("r", all(10))?)?, vec![0, 1, 2, 3, 4]);
    assert_eq!(store.append("r", batch(7, 1)?), Append::Full);
    assert_eq!(store.rows("r"), Some((5, true)));
    Ok(())
}

#[test]
fn exactly_reaching_the_cap_continues_until_more_rows_arrive() -> TestResult {
    let store = ResultStore::with_limits(Limits {
        max_rows: 4,
        ..Limits::default()
    });
    store.begin("r", schema());
    assert_eq!(store.append("r", batch(0, 4)?), Append::Continue);
    assert_eq!(store.rows("r"), Some((4, false)));
    assert_eq!(store.append("r", batch(4, 1)?), Append::Full);
    assert_eq!(store.rows("r"), Some((4, true)));
    Ok(())
}

#[test]
fn global_memory_budget_is_shared_and_freed_on_remove() -> TestResult {
    let one = batch(0, 100)?.get_array_memory_size();
    let store = ResultStore::with_limits(Limits {
        max_rows: usize::MAX,
        max_bytes: one * 3 / 2,
    });
    store.begin("a", schema());
    store.begin("b", schema());
    assert_eq!(store.append("a", batch(0, 100)?), Append::Continue);
    assert_eq!(store.memory(), one);
    assert_eq!(store.append("b", batch(0, 100)?), Append::Full);
    assert_eq!(store.rows("b"), Some((0, true)));
    assert!(store.remove("a"));
    assert_eq!(store.memory(), 0);
    store.begin("c", schema());
    assert_eq!(store.append("c", batch(0, 100)?), Append::Continue);
    Ok(())
}

#[test]
fn released_results_stop_the_stream_and_reject_windows() -> TestResult {
    let store = ResultStore::default();
    store.begin("r", schema());
    assert_eq!(store.append("r", batch(0, 2)?), Append::Continue);
    assert!(store.remove("r"));
    assert!(!store.remove("r"));
    assert_eq!(store.append("r", batch(2, 2)?), Append::Full);
    assert!(matches!(
        store.window("r", all(1)),
        Err(WindowError::UnknownResult(_))
    ));
    assert_eq!(store.memory(), 0);
    Ok(())
}

#[test]
fn mismatched_schema_truncates_instead_of_corrupting() -> TestResult {
    let store = ResultStore::default();
    store.begin("r", schema());
    let other = synthetic::batch(2, 2, 42)?;
    assert_eq!(store.append("r", other), Append::Full);
    assert_eq!(store.rows("r"), Some((0, true)));
    Ok(())
}

#[test]
fn begin_and_insert_replace_and_reaccount() -> TestResult {
    let store = ResultStore::default();
    store.begin("r", schema());
    assert_eq!(store.append("r", batch(0, 10)?), Append::Continue);
    store.begin("r", schema());
    assert_eq!(store.rows("r"), Some((0, false)));
    assert_eq!(store.memory(), 0);
    store.insert("r".to_owned(), vec![batch(0, 10)?])?;
    store.insert("r".to_owned(), vec![batch(0, 10)?])?;
    assert_eq!(store.memory(), batch(0, 10)?.get_array_memory_size());
    assert_eq!(values(store.window("r", all(1))?)?, vec![0]);
    Ok(())
}
