// SPDX-License-Identifier: Apache-2.0
//! Opt-in postcard/frame codec timings.
use criterion::{Criterion, criterion_group, criterion_main};
use openrows_host_protocol::{Control, Frame, Hello};
use std::hint::black_box;
fn codec(criterion: &mut Criterion) {
    if std::env::var("OPENROWS_BENCH").as_deref() != Ok("1") {
        return;
    }
    let dto = Control::Hello(Hello::sqlite(1));
    criterion.bench_function("control_encode", |bench| {
        bench.iter(|| Frame::control(1, 0, black_box(&dto)));
    });
    let Ok(frame) = Frame::control(1, 0, &dto) else {
        return;
    };
    criterion.bench_function("control_decode", |bench| {
        bench.iter(|| black_box(&frame).decode_control());
    });
}
criterion_group!(benches, codec);
criterion_main!(benches);
