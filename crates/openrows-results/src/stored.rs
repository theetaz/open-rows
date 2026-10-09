// SPDX-License-Identifier: Apache-2.0
//! A lazy reader over a stored result's batches, e.g. for export.
use crate::set::Slot;
use crate::shared::Shared;
use crate::store::WindowError;
use arrow_array::RecordBatch;
use std::sync::Arc;

/// The batches a result held when [`crate::ResultStore::batches`] was
/// called, in order. Spilled batches are decoded one at a time and bypass
/// the decoded-batch cache, so a long export does not evict the grid's.
///
/// Yields an `UnknownResult` error (and then ends) when the result is
/// released or replaced mid-way, and a spill error when a file cannot be read.
pub struct StoredBatches {
    shared: Arc<Shared>,
    id: String,
    generation: u64,
    next: usize,
    end: usize,
}

impl StoredBatches {
    pub(crate) fn new(shared: Arc<Shared>, (id, generation): (String, u64), end: usize) -> Self {
        Self {
            shared,
            id,
            generation,
            next: 0,
            end,
        }
    }
}

impl Iterator for StoredBatches {
    type Item = Result<RecordBatch, WindowError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next >= self.end {
            return None;
        }
        let slot = self.shared.slot((&self.id, self.generation), self.next);
        self.next = match slot {
            Ok(_) => self.next + 1,
            Err(_) => self.end,
        };
        Some(slot.and_then(|slot| match slot {
            Slot::Memory { batch, .. } => Ok(batch),
            Slot::Spilled { file, index } => file.read(index),
        }))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let left = self.end - self.next;
        (0, Some(left))
    }
}

impl std::fmt::Debug for StoredBatches {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoredBatches")
            .field("id", &self.id)
            .field("next", &self.next)
            .field("end", &self.end)
            .finish_non_exhaustive()
    }
}
