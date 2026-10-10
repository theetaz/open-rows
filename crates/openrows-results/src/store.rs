// SPDX-License-Identifier: Apache-2.0
use crate::limits::{Append, Limits};
use crate::set::ResultSet;
use crate::shared::{Shared, spill_pass};
use crate::slice::project_slice;
use crate::spill::{Spill, SpillDir};
use crate::stored::StoredBatches;
use arrow_array::cast::AsArray;
use arrow_array::types::Int64Type;
use arrow_array::{Array, ArrayRef, RecordBatch};
use arrow_ipc::writer::StreamWriter;
use arrow_schema::{ArrowError, SchemaRef};
use std::sync::{Arc, Weak};

/// A viewport request; counts are clamped, starts beyond the end are errors.
#[derive(Clone, Copy, Debug)]
pub struct Window {
    /// First row, inclusive (the end itself is a valid empty window).
    pub row_start: usize,
    /// Maximum number of rows.
    pub row_count: usize,
    /// First column, inclusive.
    pub col_start: usize,
    /// Maximum number of columns.
    pub col_count: usize,
}

/// Typed failures at the result boundary.
#[derive(Debug, thiserror::Error)]
pub enum WindowError {
    /// The identifier has no stored result.
    #[error("unknown result: {0}")]
    UnknownResult(String),
    /// A start offset exceeds the corresponding dimension.
    #[error("window start exceeds result bounds")]
    OutOfRange,
    /// Insertion requires at least one batch, all with identical schemas.
    #[error("result requires batches with a shared schema")]
    InvalidBatches,
    /// The cell holds numbers, dates or another type with no byte value.
    #[error("cell does not hold text or binary data")]
    NotBytes,
    /// Arrow rejected the schema or stream encoding.
    #[error(transparent)]
    Arrow(#[from] ArrowError),
    /// A spill file could not be written or read back.
    #[error("spilled rows could not be read or written: {0}")]
    Spill(#[from] std::io::Error),
}

/// Result ownership stays in the core; slices share the original Arrow buffers.
///
/// Thread-safe: session actors append while the UI reads windows. [`Limits`]
/// caps rows per result and the Arrow memory kept in RAM across results.
/// With a [`SpillDir`], a background thread writes the least recently used
/// results' batches to LZ4-compressed Arrow IPC files beyond the budget, and
/// reads decode them back transparently through a small cache; without one,
/// a stream that would exceed the budget stops instead.
pub struct ResultStore {
    pub(crate) shared: Arc<Shared>,
}

impl Default for ResultStore {
    fn default() -> Self {
        Self::with_limits(Limits::default())
    }
}

impl ResultStore {
    /// An empty in-memory store with explicit limits.
    #[must_use]
    pub fn with_limits(limits: Limits) -> Self {
        Self {
            shared: Arc::new(Shared::new(limits, None)),
        }
    }

    /// An empty store that spills beyond `limits.max_bytes` into `dir`.
    ///
    /// # Errors
    /// The spill thread cannot be started.
    pub fn with_spill(limits: Limits, dir: SpillDir) -> std::io::Result<Self> {
        let mut failed = None;
        let shared = Arc::new_cyclic(|weak: &Weak<Shared>| {
            let spill =
                Spill::start(dir, spill_pass(weak.clone())).map_err(|error| failed = Some(error));
            Shared::new(limits, spill.ok())
        });
        match failed {
            Some(error) => Err(error),
            None => Ok(Self { shared }),
        }
    }

    /// Insert or replace a complete result. An empty result uses a zero-row batch.
    ///
    /// # Errors
    /// Returns `InvalidBatches` for missing or inconsistent batches.
    pub fn insert(&self, id: String, batches: Vec<RecordBatch>) -> Result<(), WindowError> {
        let schema = batches.first().ok_or(WindowError::InvalidBatches)?.schema();
        if batches.iter().any(|batch| batch.schema() != schema) {
            return Err(WindowError::InvalidBatches);
        }
        batches.iter().try_fold(0usize, |rows, batch| {
            rows.checked_add(batch.num_rows())
                .ok_or(WindowError::InvalidBatches)
        })?;
        let mut result = ResultSet::new(schema, 0, self.shared.tick());
        for batch in batches {
            let bytes = batch.get_array_memory_size();
            result.push(batch, bytes);
        }
        let replaced = self.shared.write().put(id, result);
        drop(replaced);
        self.shared.spill_if_over();
        Ok(())
    }

    /// Start a streamed result with no rows, replacing any result with the same id.
    pub fn begin(&self, id: &str, schema: SchemaRef) {
        let result = ResultSet::new(schema, 0, self.shared.tick());
        let replaced = self.shared.write().put(id.to_owned(), result);
        drop(replaced);
        self.shared.changed.notify_waiters();
    }

    /// Append a batch to a streamed result.
    ///
    /// Returns [`Append::Full`] when the result hit the row cap (the batch is
    /// cut to fit), the memory budget is exhausted and the store cannot spill
    /// (the batch is dropped), the id is unknown (released) or the schema
    /// does not match. Never waits for disk I/O: spilling runs on its own thread.
    #[must_use]
    pub fn append(&self, id: &str, batch: RecordBatch) -> Append {
        let (flow, over) = self.shared.append(id, batch);
        if over {
            self.shared.request_spill();
        }
        flow
    }

    /// Drop a result, its memory and its spill files; false when the id is unknown.
    #[expect(
        clippy::must_use_candidate,
        reason = "releasing an already released result is not an error"
    )]
    pub fn remove(&self, id: &str) -> bool {
        let removed = self.shared.write().take(id);
        self.shared.changed.notify_waiters();
        removed.is_some_and(|result| {
            self.shared.cache().forget(&result.spill_files());
            true
        })
    }

    /// Drop every result and delete the spill directory (app exit).
    pub fn close(&self) {
        let results = {
            let mut inner = self.shared.write();
            inner.bytes = 0;
            std::mem::take(&mut inner.results)
        };
        drop(results);
        self.shared.changed.notify_waiters();
        self.shared.cache().clear();
        if let Some(spill) = &self.shared.spill {
            spill.close();
        }
    }

    /// Wait until the spill thread has caught up with the memory budget.
    pub fn settle(&self) {
        if let Some(spill) = &self.shared.spill {
            spill.wait_idle();
        }
    }

    /// Rows stored so far and whether a limit cut the result short.
    #[must_use]
    pub fn rows(&self, id: &str) -> Option<(usize, bool)> {
        self.shared
            .read()
            .results
            .get(id)
            .map(|result| (result.rows, result.truncated))
    }

    /// The schema and a lazy reader over the batches stored so far, e.g. for
    /// export; `None` when the id is unknown. Spilled batches are decoded one
    /// at a time as the reader advances. See [`ResultStore::rows`] for
    /// whether a limit cut the result short.
    #[must_use]
    pub fn batches(&self, id: &str) -> Option<(SchemaRef, StoredBatches)> {
        let inner = self.shared.read();
        let result = inner.results.get(id)?;
        let reader = StoredBatches::new(
            Arc::clone(&self.shared),
            (id.to_owned(), result.generation),
            result.slot_count(),
        );
        Some((result.schema.clone(), reader))
    }

    /// The first row's first value as an integer, e.g. a `count(*)`; `None`
    /// when the result is unknown, empty, NULL there or not a 64-bit integer.
    #[must_use]
    pub fn first_integer(&self, id: &str) -> Option<i64> {
        let slot = self.shared.read().results.get(id)?.slot(0)?.clone();
        let batch = self.shared.load(slot).ok()?;
        let column = batch.columns().first()?.as_primitive_opt::<Int64Type>()?;
        column.is_valid(0).then(|| column.value(0))
    }

    /// Arrow memory held in RAM by all results (spilled batches excluded).
    #[must_use]
    pub fn memory(&self) -> usize {
        self.shared.read().bytes
    }

    /// Slice rows and columns and encode an uncompressed Arrow IPC stream.
    ///
    /// # Errors
    /// Returns typed identifier, bounds, Arrow encoding or spill read errors.
    pub fn window(&self, id: &str, window: Window) -> Result<Vec<u8>, WindowError> {
        let plan = self.shared.plan(id, window)?;
        let mut bytes = Vec::new();
        let mut writer = StreamWriter::try_new(&mut bytes, &plan.schema)?;
        for (slot, rows) in plan.slices {
            let batch = self.shared.load(slot)?;
            writer.write(&project_slice(
                &batch,
                plan.schema.clone(),
                &plan.indices,
                rows,
            )?)?;
        }
        writer.finish()?;
        Ok(bytes)
    }

    /// The column array holding one cell, and the cell's index inside that
    /// array's batch. Shares the buffers; the lock is released on return.
    pub(crate) fn cell_column(
        &self,
        id: &str,
        row: usize,
        column: usize,
    ) -> Result<(ArrayRef, usize), WindowError> {
        let now = self.shared.tick();
        let (slot, index) = {
            let inner = self.shared.read();
            let result = inner.get(id)?;
            result.touch(now);
            if column >= result.schema.fields().len() {
                return Err(WindowError::OutOfRange);
            }
            let (slot, index) = result.locate(row).ok_or(WindowError::OutOfRange)?;
            (slot.clone(), index)
        };
        let batch = self.shared.load(slot)?;
        Ok((Arc::clone(batch.column(column)), index))
    }

    #[cfg(test)]
    pub(crate) fn spilled_batches(&self, id: &str) -> Option<usize> {
        let inner = self.shared.read();
        let result = inner.results.get(id)?;
        Some(result.slot_count() - result.in_memory().count())
    }

    #[cfg(test)]
    pub(crate) fn cached_batches(&self) -> usize {
        self.shared.cache().len()
    }
}
