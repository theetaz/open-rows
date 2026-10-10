// SPDX-License-Identifier: Apache-2.0
//! JSON storage-value parsing plus shared `SQLite` Arrow batching throughput.
use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use openrows_driver_libsql::Value;
use openrows_driver_sqlite::decode::{BatchBuffer, ColumnSpec};
use std::sync::Arc;
fn decode(c: &mut Criterion) {
    if std::env::var("OPENROWS_BENCH").as_deref() != Ok("1") {
        return;
    }
    let row = r#"[{"type":"integer","value":"9223372036854775807"},{"type":"float","value":1.5},{"type":"text","value":"සිංහල"},{"type":"blob","base64":"AP8="},{"type":"null"}]"#;
    let columns: Arc<[ColumnSpec]> = ["INTEGER", "REAL", "TEXT", "BLOB", "TEXT"]
        .into_iter()
        .enumerate()
        .map(|(index, declared)| ColumnSpec {
            name: index.to_string(),
            declared: Some(declared.into()),
        })
        .collect();
    let mut group = c.benchmark_group("decode_hrana");
    group.throughput(Throughput::Elements(1000));
    group.bench_function("200x5", |b| {
        b.iter(|| -> Result<_, Box<dyn std::error::Error>> {
            let mut buffer = BatchBuffer::new(Arc::clone(&columns));
            for _ in 0..200 {
                let values: Vec<Value> = serde_json::from_str(std::hint::black_box(row))?;
                let cells = values
                    .into_iter()
                    .enumerate()
                    .map(|(index, value)| value.into_cell(index))
                    .collect::<Result<Vec<_>, _>>()?;
                buffer.push(cells);
            }
            Ok(std::hint::black_box(buffer.finish()?))
        });
    });
    group.finish();
}
criterion_group!(benches, decode);
criterion_main!(benches);
