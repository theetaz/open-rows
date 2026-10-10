// SPDX-License-Identifier: Apache-2.0
//! Store pressure is observable without changing in-process append behavior.
use super::*;
use std::sync::atomic::Ordering;
#[test]
fn admission_pauses_above_budget_and_resumes_only_at_spill_target()
-> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let batch = synthetic::batch(200, 20, 42)?;
    let bytes = batch.get_array_memory_size();
    let store = ResultStore::with_spill(
        Limits {
            max_rows: usize::MAX,
            max_bytes: bytes * 2,
        },
        SpillDir::create(root.path())?,
    )?;
    store.begin("r", batch.schema());
    // Bypass the wake-up to inspect pressure before a deterministic manual spill pass.
    for _ in 0..3 {
        assert_eq!(store.shared.append("r", batch.clone()).0, Append::Continue);
    }
    assert_eq!(store.admission("r"), Admission::Pause);
    store.shared.request_spill();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    assert_eq!(runtime.block_on(store.resume("r")), Admission::Ready);
    assert!(store.memory() <= bytes * 2 - bytes / 2);
    assert_eq!(store.rows("r"), Some((600, false)));
    Ok(())
}
#[test]
fn failed_spill_and_released_results_stop_without_more_allocation()
-> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let batch = synthetic::batch(200, 20, 42)?;
    let store = ResultStore::with_spill(
        Limits {
            max_rows: usize::MAX,
            max_bytes: 1,
        },
        SpillDir::create(root.path())?,
    )?;
    store.begin("r", batch.schema());
    assert_eq!(store.shared.append("r", batch.clone()).0, Append::Continue);
    let directory = std::fs::read_dir(root.path())?
        .next()
        .ok_or("no run directory")??
        .path();
    std::fs::remove_dir_all(directory)?;
    store.shared.request_spill();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    assert_eq!(runtime.block_on(store.resume("r")), Admission::Stop);
    assert_eq!(store.admission("r"), Admission::Stop);
    let memory = store.memory();
    assert_eq!(store.append("r", batch), Append::Full);
    assert_eq!(store.memory(), memory);
    store.remove("r");
    assert_eq!(store.admission("r"), Admission::Stop);
    assert!(store.shared.pressured.load(Ordering::SeqCst));
    store.begin("fresh", synthetic::batch(1, 1, 42)?.schema());
    assert_eq!(store.admission("fresh"), Admission::Ready);
    Ok(())
}
