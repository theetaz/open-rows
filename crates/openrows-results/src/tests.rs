// SPDX-License-Identifier: Apache-2.0
use super::*;
use arrow_array::{Array, RecordBatch};
use arrow_ipc::reader::StreamReader;
use proptest::prelude::*;
use std::io::Cursor;

fn source_store() -> Result<(RecordBatch, ResultStore), Box<dyn std::error::Error>> {
    let source = synthetic::batch(73, 20, 42)?;
    let store = ResultStore::default();
    store.insert(
        "source".into(),
        vec![source.slice(0, 31), source.slice(31, 42)],
    )?;
    Ok((source, store))
}

fn check_window(window: Window) -> Result<(), Box<dyn std::error::Error>> {
    let (source, store) = source_store()?;
    let bytes = store.window("source", window)?;
    let reader = StreamReader::try_new(Cursor::new(bytes), None)?;
    let cols = window.col_count.min(20 - window.col_start);
    assert_eq!(reader.schema().fields().len(), cols);
    let rows = window.row_count.min(73 - window.row_start);
    let mut actual_rows = 0;
    for batch in reader {
        let batch = batch?;
        for col in 0..cols {
            let expected = source
                .column(col + window.col_start)
                .slice(window.row_start + actual_rows, batch.num_rows());
            assert_eq!(batch.column(col).to_data(), expected.to_data());
            assert_eq!(
                batch.schema().field(col),
                source.schema().field(col + window.col_start)
            );
        }
        actual_rows += batch.num_rows();
    }
    assert_eq!(actual_rows, rows);
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig { rng_seed: proptest::test_runner::RngSeed::Fixed(42), ..ProptestConfig::default() })]
    #[test]
    fn random_windows_match_source(row_start in 0usize..74, row_count in 0usize..150,
                                  col_start in 0usize..21, col_count in 0usize..40) {
        check_window(Window { row_start, row_count, col_start, col_count }).unwrap();
    }
}

#[test]
fn empty_clamped_and_overflowing_windows() -> Result<(), Box<dyn std::error::Error>> {
    for (row_start, row_count, col_start, col_count) in [
        (73, 50, 0, 20),
        (0, 0, 0, 20),
        (20, 10, 20, 0),
        (20, usize::MAX, 5, usize::MAX),
        (0, 73, 0, 20),
    ] {
        check_window(Window {
            row_start,
            row_count,
            col_start,
            col_count,
        })?;
    }
    Ok(())
}

#[test]
fn errors_are_typed() -> Result<(), Box<dyn std::error::Error>> {
    let (_, store) = source_store()?;
    let window = Window {
        row_start: 0,
        row_count: 10,
        col_start: 0,
        col_count: 20,
    };
    assert!(matches!(
        store.window("missing", window),
        Err(WindowError::UnknownResult(_))
    ));
    assert!(matches!(
        store.window(
            "source",
            Window {
                row_start: 74,
                ..window
            }
        ),
        Err(WindowError::OutOfRange)
    ));
    assert!(matches!(
        store.window(
            "source",
            Window {
                col_start: 21,
                ..window
            }
        ),
        Err(WindowError::OutOfRange)
    ));
    assert!(matches!(
        store.insert("empty".into(), vec![]),
        Err(WindowError::InvalidBatches)
    ));
    assert!(matches!(
        store.insert(
            "bad".into(),
            vec![synthetic::batch(2, 1, 42)?, synthetic::batch(2, 2, 42)?]
        ),
        Err(WindowError::InvalidBatches)
    ));
    Ok(())
}

#[test]
fn deterministic_source_and_zero_rows() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(synthetic::batch(73, 20, 42)?, synthetic::batch(73, 20, 42)?);
    assert_ne!(synthetic::batch(73, 20, 42)?, synthetic::batch(73, 20, 43)?);
    let store = ResultStore::default();
    store.insert("empty".into(), vec![synthetic::batch(0, 20, 42)?])?;
    let bytes = store.window(
        "empty",
        Window {
            row_start: 0,
            row_count: 1,
            col_start: 0,
            col_count: 20,
        },
    )?;
    assert_eq!(StreamReader::try_new(Cursor::new(bytes), None)?.count(), 0);
    Ok(())
}

#[test]
fn rust_encoder_golden_fixture() -> Result<(), Box<dyn std::error::Error>> {
    let (_, store) = source_store()?;
    let bytes = store.window(
        "source",
        Window {
            row_start: 3,
            row_count: 36,
            col_start: 0,
            col_count: 20,
        },
    )?;
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/desktop/src/results/fixtures/mixed.arrow");
    if std::env::var_os("OPENROWS_UPDATE_FIXTURE").is_some() {
        std::fs::write(&path, &bytes)?;
    }
    assert_eq!(
        std::fs::read(path)?,
        bytes,
        "regenerate with OPENROWS_UPDATE_FIXTURE=1 cargo test -p openrows-results rust_encoder_golden_fixture"
    );
    Ok(())
}
