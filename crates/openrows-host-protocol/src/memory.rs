// SPDX-License-Identifier: Apache-2.0
//! Shared Arrow allocations count once, including retained slices/dictionaries.
use arrow_array::RecordBatch;
use arrow_buffer::Buffer;
use arrow_data::ArrayData;
use std::collections::HashSet;
/// Retained Arrow memory with shared IPC bodies counted once per allocation.
#[must_use]
pub fn batch_memory(batch: &RecordBatch) -> usize {
    let mut pointers = HashSet::new();
    batch
        .columns()
        .iter()
        .map(|array| data_memory(&array.to_data(), &mut pointers))
        .sum()
}
pub(crate) fn data_memory(data: &ArrayData, pointers: &mut HashSet<usize>) -> usize {
    let mut bytes = size_of::<ArrayData>() + size_of_val(data.buffers());
    for buffer in data.buffers() {
        bytes = bytes.saturating_add(buffer_memory(buffer, pointers));
    }
    if let Some(nulls) = data.nulls() {
        bytes = bytes.saturating_add(buffer_memory(nulls.buffer(), pointers));
    }
    for child in data.child_data() {
        bytes = bytes.saturating_add(data_memory(child, pointers));
    }
    bytes
}
fn buffer_memory(buffer: &Buffer, pointers: &mut HashSet<usize>) -> usize {
    if pointers.insert(buffer.data_ptr().as_ptr() as usize) {
        buffer.capacity()
    } else {
        0
    }
}
