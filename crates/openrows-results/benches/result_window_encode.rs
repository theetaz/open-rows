// SPDX-License-Identifier: Apache-2.0
//! Slice + uncompressed IPC encoding, excluding synthetic source construction.
use criterion::Criterion;
use openrows_results::{ResultStore, Window, synthetic};
use std::hint::black_box;

fn window_encode(criterion: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
    let mut group = criterion.benchmark_group("result_window_encode");
    for (rows, cols) in [(256, 20), (1024, 20), (256, 100)] {
        let source = synthetic::batch(synthetic::SPIKE_ROWS, cols, synthetic::SPIKE_SEED)?;
        let mut store = ResultStore::default();
        store.insert("bench".to_owned(), vec![source])?;
        let window = Window {
            row_start: 543_219,
            row_count: rows,
            col_start: 0,
            col_count: cols,
        };
        group.bench_function(format!("{rows}x{cols}"), |bench| {
            bench.iter(|| {
                black_box(
                    store
                        .window("bench", black_box(window))
                        .unwrap_or_else(|_| std::process::exit(1)),
                )
            });
        });
    }
    group.finish();
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut criterion = Criterion::default().configure_from_args();
    window_encode(&mut criterion)?;
    criterion.final_summary();
    Ok(())
}
