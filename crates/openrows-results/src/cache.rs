// SPDX-License-Identifier: Apache-2.0
//! Recently decoded spilled batches, so scrolling within spilled rows
//! decodes each batch once instead of once per window.
use arrow_array::RecordBatch;
use std::collections::VecDeque;

/// Decoded batches kept; a grid viewport spans one or two batches.
pub(crate) const CACHED_BATCHES: usize = 8;

/// (spill file id, batch index in the file)
type Key = (u64, usize);

/// Least-recently-used decoded batches, most recent first. Kept outside
/// the memory budget: it is bounded by [`CACHED_BATCHES`].
#[derive(Default)]
pub(crate) struct DecodedCache {
    entries: VecDeque<(Key, RecordBatch)>,
}

impl DecodedCache {
    /// The cached batch, now the most recently used.
    pub(crate) fn get(&mut self, key: Key) -> Option<RecordBatch> {
        let position = self.entries.iter().position(|(cached, _)| *cached == key)?;
        let entry = self.entries.remove(position)?;
        let batch = entry.1.clone();
        self.entries.push_front(entry);
        Some(batch)
    }

    /// Remember a decoded batch, dropping the least recently used beyond the cap.
    pub(crate) fn put(&mut self, key: Key, batch: RecordBatch) {
        self.entries.retain(|(cached, _)| *cached != key);
        self.entries.push_front((key, batch));
        self.entries.truncate(CACHED_BATCHES);
    }

    /// Forget every batch decoded from the given files.
    pub(crate) fn forget(&mut self, files: &[u64]) {
        self.entries.retain(|((file, _), _)| !files.contains(file));
    }

    /// Forget everything.
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }
}
