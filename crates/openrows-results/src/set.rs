// SPDX-License-Identifier: Apache-2.0
//! One stored result: its batches, where each lives and the row offsets.
use crate::spill::SpillFile;
use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Where one batch of a result lives. Cloning shares the buffers or the file.
#[derive(Clone)]
pub(crate) enum Slot {
    /// In RAM; `bytes` is its share of the memory budget.
    Memory { batch: RecordBatch, bytes: usize },
    /// Batch `index` of a spill file.
    Spilled { file: Arc<SpillFile>, index: usize },
}

/// A result's batches in arrival order with the first row of each, so a row
/// is found by binary search instead of a walk over the batches.
pub(crate) struct ResultSet {
    pub(crate) schema: SchemaRef,
    slots: Vec<Slot>,
    starts: Vec<usize>,
    pub(crate) rows: usize,
    /// Bytes of the batches still in RAM.
    pub(crate) bytes: usize,
    pub(crate) truncated: bool,
    /// Tells a replaced result apart from its successor under the same id.
    pub(crate) generation: u64,
    last_used: AtomicU64,
}

impl ResultSet {
    pub(crate) fn new(schema: SchemaRef, generation: u64, now: u64) -> Self {
        Self {
            schema,
            slots: Vec::new(),
            starts: Vec::new(),
            rows: 0,
            bytes: 0,
            truncated: false,
            generation,
            last_used: AtomicU64::new(now),
        }
    }

    /// Keep `batch` in RAM; returns the bytes it adds. Empty batches hold
    /// no rows and are skipped.
    pub(crate) fn push(&mut self, batch: RecordBatch, bytes: usize) -> usize {
        if batch.num_rows() == 0 {
            return 0;
        }
        self.starts.push(self.rows);
        self.rows += batch.num_rows();
        self.bytes += bytes;
        self.slots.push(Slot::Memory { batch, bytes });
        bytes
    }

    /// Record a read or write for least-recently-used eviction.
    pub(crate) fn touch(&self, now: u64) {
        self.last_used.store(now, Ordering::Relaxed);
    }

    pub(crate) fn last_used(&self) -> u64 {
        self.last_used.load(Ordering::Relaxed)
    }

    pub(crate) fn slot(&self, index: usize) -> Option<&Slot> {
        self.slots.get(index)
    }

    pub(crate) fn slot_count(&self) -> usize {
        self.slots.len()
    }

    /// The slot holding `row` and the row's index inside that batch.
    pub(crate) fn locate(&self, row: usize) -> Option<(&Slot, usize)> {
        if row >= self.rows {
            return None;
        }
        let index = self.starts.partition_point(|&start| start <= row) - 1;
        Some((self.slots.get(index)?, row - self.starts.get(index)?))
    }

    /// The slots overlapping `rows`, each with the row range inside it.
    pub(crate) fn slices(&self, rows: &Range<usize>) -> Vec<(Slot, Range<usize>)> {
        let mut out = Vec::new();
        if rows.is_empty() || rows.start >= self.rows {
            return out;
        }
        let first = self.starts.partition_point(|&start| start <= rows.start) - 1;
        for (index, slot) in self.slots.iter().enumerate().skip(first) {
            let start = self.starts.get(index).copied().unwrap_or(self.rows);
            if start >= rows.end {
                break;
            }
            let end = self.starts.get(index + 1).copied().unwrap_or(self.rows);
            let range = rows.start.max(start) - start..rows.end.min(end) - start;
            out.push((slot.clone(), range));
        }
        out
    }

    /// Batches still in RAM, oldest first, with their slot index and bytes.
    pub(crate) fn in_memory(&self) -> impl Iterator<Item = (usize, &RecordBatch, usize)> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| match slot {
                Slot::Memory { batch, bytes } => Some((index, batch, *bytes)),
                Slot::Spilled { .. } => None,
            })
    }

    /// Point slot `index` at its copy on disk; returns the bytes freed.
    pub(crate) fn spill_slot(&mut self, index: usize, spilled: Slot) -> usize {
        let Some(slot) = self.slots.get_mut(index) else {
            return 0;
        };
        let Slot::Memory { bytes, .. } = *slot else {
            return 0;
        };
        *slot = spilled;
        self.bytes = self.bytes.saturating_sub(bytes);
        bytes
    }

    /// Ids of the spill files this result reads from.
    pub(crate) fn spill_files(&self) -> Vec<u64> {
        let mut ids: Vec<u64> = self
            .slots
            .iter()
            .filter_map(|slot| match slot {
                Slot::Spilled { file, .. } => Some(file.id()),
                Slot::Memory { .. } => None,
            })
            .collect();
        ids.dedup();
        ids
    }
}
