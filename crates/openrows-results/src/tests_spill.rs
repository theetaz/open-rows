// SPDX-License-Identifier: Apache-2.0
//! Spill to disk: round trip, eviction order, accounting and clean-up.
use super::*;
use arrow_array::{Array, Int64Array, RecordBatch};
use arrow_ipc::reader::StreamReader;
use arrow_schema::{DataType, Field, Schema, SchemaRef};
use std::io::Cursor;
use std::path::Path;
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

/// Bytes of one 100-row test batch.
fn unit() -> Result<usize, arrow_schema::ArrowError> {
    Ok(batch(0, 100)?.get_array_memory_size())
}

fn spilling(root: &Path, max_bytes: usize) -> Result<ResultStore, std::io::Error> {
    let limits = Limits {
        max_rows: usize::MAX,
        max_bytes,
    };
    ResultStore::with_spill(limits, SpillDir::create(root)?)
}

/// Append `batches` batches of 100 rows numbered from 0.
fn stream(store: &ResultStore, id: &str, batches: i64) -> TestResult {
    store.begin(id, schema());
    for index in 0..batches {
        assert_eq!(store.append(id, batch(index * 100, 100)?), Append::Continue);
    }
    Ok(())
}

fn values(
    store: &ResultStore,
    id: &str,
    rows: (usize, usize),
) -> Result<Vec<i64>, Box<dyn std::error::Error>> {
    let window = Window {
        row_start: rows.0,
        row_count: rows.1,
        col_start: 0,
        col_count: 1,
    };
    let mut out = Vec::new();
    for batch in StreamReader::try_new(Cursor::new(store.window(id, window)?), None)? {
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

/// Spill files (not the lock file) in the run directory.
fn spill_files(dir: &Path) -> Result<usize, std::io::Error> {
    Ok(std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "arrows"))
        .count())
}

fn run_dir(root: &Path) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let entry = std::fs::read_dir(root)?.next().ok_or("no run dir")??;
    Ok(entry.path())
}

#[test]
fn spilled_rows_read_back_through_windows_cells_and_batches() -> TestResult {
    let root = tempfile::tempdir()?;
    let store = spilling(root.path(), unit()? * 4)?;
    stream(&store, "r", 20)?;
    store.settle();
    assert!(store.memory() <= unit()? * 4, "memory {}", store.memory());
    assert!(store.spilled_batches("r").ok_or("missing")? >= 16);
    assert_eq!(store.rows("r"), Some((2_000, false)));
    let expected: Vec<i64> = (0..2_000).collect();
    assert_eq!(values(&store, "r", (0, 2_000))?, expected);
    assert_eq!(
        values(&store, "r", (150, 120))?,
        (150..270).collect::<Vec<_>>()
    );
    assert_eq!(store.first_integer("r"), Some(0));
    let info = store.cell_info("r", 1_234, 0)?;
    assert_eq!(info.kind, CellKind::Other);
    let (_, batches) = store.batches("r").ok_or("missing")?;
    let mut read = Vec::new();
    for batch in batches {
        let batch = batch?;
        let column = batch
            .column(0)
            .as_any()
            .downcast_ref::<Int64Array>()
            .ok_or("not Int64")?;
        read.extend(column.iter().flatten());
    }
    assert_eq!(read, expected);
    Ok(())
}

#[test]
fn least_recently_used_results_spill_first() -> TestResult {
    let root = tempfile::tempdir()?;
    let store = spilling(root.path(), unit()? * 10)?;
    let four = |from: i64| {
        (from..from + 4)
            .map(|index| batch(index * 100, 100))
            .collect::<Result<Vec<_>, _>>()
    };
    store.insert("a".to_owned(), four(0)?)?;
    store.insert("b".to_owned(), four(4)?)?;
    store.settle();
    assert_eq!(store.memory(), unit()? * 8);
    values(&store, "a", (0, 1))?;
    store.insert("c".to_owned(), four(8)?)?;
    store.settle();
    // 12 units over a budget of 10 spill down to 7.5: all of b (least
    // recently used), then a's oldest batch; c, streamed last, stays.
    assert_eq!(store.spilled_batches("b"), Some(4));
    assert_eq!(store.spilled_batches("a"), Some(1));
    assert_eq!(store.spilled_batches("c"), Some(0));
    assert_eq!(store.memory(), unit()? * 7);
    Ok(())
}

#[test]
fn release_and_close_delete_spill_files() -> TestResult {
    let root = tempfile::tempdir()?;
    let store = spilling(root.path(), unit()? * 2)?;
    stream(&store, "a", 6)?;
    store.settle();
    let dir = run_dir(root.path())?;
    assert!(spill_files(&dir)? > 0);
    values(&store, "a", (0, 600))?;
    assert!(store.cached_batches() > 0);
    assert!(store.remove("a"));
    assert_eq!(spill_files(&dir)?, 0);
    assert_eq!(store.cached_batches(), 0);
    assert_eq!(store.memory(), 0);
    stream(&store, "b", 6)?;
    store.settle();
    assert!(spill_files(&dir)? > 0);
    store.close();
    assert!(!dir.exists());
    assert_eq!(store.rows("b"), None);
    Ok(())
}

#[test]
fn startup_sweeps_only_ended_runs() -> TestResult {
    let root = tempfile::tempdir()?;
    let live = SpillDir::create(root.path())?;
    let stale = root.path().join("run-1-1");
    std::fs::create_dir(&stale)?;
    std::fs::write(stale.join(".lock"), b"")?;
    std::fs::write(stale.join("0.arrows"), b"rows")?;
    let unlocked = root.path().join("run-2-2");
    std::fs::create_dir(&unlocked)?;
    let unrelated = root.path().join("keep");
    std::fs::create_dir(&unrelated)?;
    let next = SpillDir::create(root.path())?;
    assert!(!stale.exists());
    assert!(!unlocked.exists());
    assert!(live.path().exists());
    assert!(unrelated.exists());
    assert_eq!(spill::sweep(root.path()), 0);
    let next_path = next.path().to_owned();
    drop(next);
    assert!(!next_path.exists());
    Ok(())
}

#[cfg(unix)]
#[test]
fn spill_files_and_directories_are_private() -> TestResult {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir()?;
    let store = spilling(root.path(), unit()?)?;
    stream(&store, "a", 4)?;
    store.settle();
    let dir = run_dir(root.path())?;
    assert_eq!(std::fs::metadata(&dir)?.permissions().mode() & 0o777, 0o700);
    let mut checked = 0;
    for entry in std::fs::read_dir(&dir)? {
        let mode = entry?.metadata()?.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        checked += 1;
    }
    assert!(checked >= 2, "lock file and at least one spill file");
    Ok(())
}

#[test]
fn decoded_cache_holds_at_most_eight_batches() -> TestResult {
    let root = tempfile::tempdir()?;
    let store = spilling(root.path(), unit()?)?;
    stream(&store, "a", 30)?;
    store.settle();
    assert_eq!(values(&store, "a", (0, 3_000))?.len(), 3_000);
    assert_eq!(store.cached_batches(), cache::CACHED_BATCHES);
    Ok(())
}

#[test]
fn released_result_ends_a_stored_reader_with_an_error() -> TestResult {
    let root = tempfile::tempdir()?;
    let store = spilling(root.path(), unit()?)?;
    stream(&store, "a", 4)?;
    store.settle();
    let (_, mut batches) = store.batches("a").ok_or("missing")?;
    assert_eq!(batches.next().ok_or("no batch")??.num_rows(), 100);
    assert!(store.remove("a"));
    assert!(matches!(
        batches.next(),
        Some(Err(WindowError::UnknownResult(_)))
    ));
    assert!(batches.next().is_none());
    Ok(())
}

#[test]
fn without_a_spill_dir_the_budget_still_stops_the_stream() -> TestResult {
    let store = ResultStore::with_limits(Limits {
        max_rows: usize::MAX,
        max_bytes: unit()? * 2,
    });
    store.begin("a", schema());
    assert_eq!(store.append("a", batch(0, 100)?), Append::Continue);
    assert_eq!(store.append("a", batch(100, 100)?), Append::Continue);
    assert_eq!(store.append("a", batch(200, 100)?), Append::Full);
    assert_eq!(store.rows("a"), Some((200, true)));
    Ok(())
}

/// Ten million rows through a 4 MiB budget: memory stays bounded and the
/// first, middle and last rows read back.
#[test]
fn ten_million_rows_stream_within_a_small_budget() -> TestResult {
    const ROWS: i64 = 10_000_000;
    const BATCH: i64 = 250_000;
    let root = tempfile::tempdir()?;
    let budget = 4 << 20;
    let store = spilling(root.path(), budget)?;
    store.begin("big", schema());
    for start in (0..ROWS).step_by(usize::try_from(BATCH)?) {
        assert_eq!(store.append("big", batch(start, BATCH)?), Append::Continue);
    }
    store.settle();
    assert!(store.memory() <= budget);
    assert_eq!(store.rows("big"), Some((10_000_000, false)));
    assert_eq!(values(&store, "big", (0, 2))?, vec![0, 1]);
    assert_eq!(values(&store, "big", (5_000_000, 1))?, vec![5_000_000]);
    assert_eq!(values(&store, "big", (9_999_999, 5))?, vec![9_999_999]);
    Ok(())
}
